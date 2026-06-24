use std::{
    collections::HashMap,
    fmt::Display,
    ops::Deref,
    path::{Path, PathBuf},
    sync::Arc,
};

use indexmap::IndexMap;
use reikland::DeserializerConfig;
use roaring::RoaringBitmap;
use serde::{Deserialize, Serialize, de::DeserializeSeed};
use snafu::{ResultExt, Snafu};

use crate::infinite_fusion::{
    abilities::AbilityDex,
    bootstrap::{Bootstrap, MoveOption, SpeciesOption, StatBounds, named_ids},
    encounters::{EncounterMode, Encounters, MapNames},
    filters::{
        SpeedCurve, StatRange,
        ability_filter::AbilityFilterIndex,
        custom_sprite_filter::CustomSpriteIndex,
        move_filter::MoveFilterIndex,
        stat_filter::StatIndex,
        type_filter::{TypeFilterIndex, fused_types},
    },
    items::{ItemDex, ItemDexDeser, ItemId},
    moves::{MoveDex, MoveDexDeser, MoveId},
    species::{
        SpeciesDex, SpeciesDexDeser, SpeciesId, base_stats::BaseStats, name_halves::NameMap,
    },
    types::{TypeDex, TypeId},
};

pub(crate) mod abilities;
pub(crate) mod area;
pub(crate) mod bootstrap;
pub(crate) mod encounters;
pub(crate) mod expert_moves;
pub(crate) mod filters;
pub(crate) mod inspect;
pub(crate) mod items;
pub(crate) mod legendaries;
pub(crate) mod map_encounters;
pub(crate) mod move_card;
pub(crate) mod moves;
pub(crate) mod settings_data;
pub(crate) mod species;
pub(crate) mod types;

// maybe just Box::leak this whole thing since the core data is immutable and it's going to get shared between threads??
/// All data across every fusion. big old type since it's multiple indexmaps so ideally get some pointer around it
#[derive(Debug)]
pub(crate) struct InfiniteFusionDex {
    abilities: AbilityDex,
    encounters: Encounters,
    items: ItemDex,
    moves: MoveDex,
    species: SpeciesDex,
    types: TypeDex,
    stat_index: StatIndex,
    type_index: TypeFilterIndex,
    ability_index: AbilityFilterIndex,
    move_index: MoveFilterIndex,
    custom_sprite_index: CustomSpriteIndex,
    /// Reverse map of move -> the machine (TM/HM) item that teaches it. A move in a species'
    /// determines between move tutor only moves and tm moves
    machine_moves: HashMap<MoveId, ItemId>,
    /// Routes where each machine (TM/HM) item is found in the world, scraped from map scripts
    tm_locations: HashMap<ItemId, Box<[Arc<str>]>>,
    /// Routes where a tutor for each tutor-taught move lives, scraped from map scripts
    tutor_locations: HashMap<MoveId, Box<[Arc<str>]>>,
    /// Move Expert signature-move rules, scraped from `FusionMoveTutor.rb`
    expert_moves: Box<[expert_moves::ExpertMove]>,
    /// Where the regular and legendary Move Expert NPCs are, scraped from map scripts
    expert_locations: expert_moves::ExpertLocations,
    /// Route name -> lowest map id bearing it, so the area picker can order routes by map id and hopefully be at least close to progress order
    route_order: HashMap<Arc<str>, u16>,
    /// Species indices (by `SpeciesId`) flagged legendary in the game's `LEGENDARIES_LIST` for the `exclude_legendaries` filter
    legendaries: RoaringBitmap,
    /// highest in-game dex number this game can actually fuse (`None` = no cap)
    max_fusable_id: Option<u16>,
    /// local-ish cache of all species stats to speed up search, costs like 3kb of ram to have this on standby
    base_stats: Box<[BaseStats]>,
    /// per-stat cumulative ranks, so `BalancedSynergy` does O(1) lookups instead of O(value) scans
    rank: Box<[[f32; 256]; 6]>,
    /// speed tier calculation curve for this game data
    speed_curve: SpeedCurve,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::EnumString)]
pub enum GameVersion {
    Kanto,
    Hoenn,
}

impl Display for GameVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GameVersion::Kanto => "Kanto",
            GameVersion::Hoenn => "Hoenn",
        }
        .fmt(f)
    }
}

impl GameVersion {
    // kanto species.dat contains species not actually in Kanto
    /// Highest in-game dex number actually fusable in this game
    pub fn max_fusable_id(self) -> Option<u16> {
        match self {
            GameVersion::Kanto => Some(501),
            GameVersion::Hoenn => Some(572), // Gastrodon E/W and shellos E/w have no autogen sprites so this cuts them off
        }
    }
}

#[derive(Debug, Snafu)]
pub enum LoadInfiniteFusionDexError {
    #[snafu(display("failed to read game data file {}", path.display()))]
    ReadFile {
        source: std::io::Error,
        path: PathBuf,
    },

    #[snafu(display("failed to deserialize game data"))]
    Deserialize {
        source: reikland::MarshalDeserializeError,
    },

    #[snafu(display("failed to load the split-names file"))]
    NameMap {
        source: species::name_halves::FromFileError,
    },
}

impl InfiniteFusionDex {
    pub fn from_path<P: AsRef<Path>>(
        base_path: P,
        game_version: GameVersion,
    ) -> Result<Self, LoadInfiniteFusionDexError> {
        let base = base_path.as_ref();

        let read_dat = |relative: &Path| -> Result<Vec<u8>, LoadInfiniteFusionDexError> {
            let path = base.join(relative);
            let bytes = std::fs::read(&path).context(ReadFileSnafu { path })?;
            Ok(maybe_decrypt(bytes))
        };

        let types = reikland::from_bytes_with_config::<TypeDex>(
            &read_dat(TypeDex::relative_path())?,
            DeserializerConfig::opinionated(),
        )
        .context(DeserializeSnafu)?;

        let moves = {
            let data = read_dat(MoveDex::relative_path())?;
            let mut de =
                reikland::Deserializer::with_config(&data, DeserializerConfig::opinionated())
                    .context(DeserializeSnafu)?;
            MoveDexDeser(&types)
                .deserialize(&mut de)
                .context(DeserializeSnafu)?
        };

        let abilities = reikland::from_bytes_with_config::<AbilityDex>(
            &read_dat(AbilityDex::relative_path())?,
            DeserializerConfig::opinionated(),
        )
        .context(DeserializeSnafu)?;

        let items = {
            let data = read_dat(ItemDex::relative_path())?;
            let mut de =
                reikland::Deserializer::with_config(&data, DeserializerConfig::opinionated())
                    .context(DeserializeSnafu)?;
            ItemDexDeser(&moves)
                .deserialize(&mut de)
                .context(DeserializeSnafu)?
        };

        let name_map_path = match game_version {
            GameVersion::Kanto => NameMap::relative_path(),
            GameVersion::Hoenn => NameMap::relative_path_hoenn(),
        };
        let name_map = NameMap::from_file(base.join(name_map_path)).context(NameMapSnafu)?;

        let species = {
            let data = read_dat(SpeciesDex::relative_path())?;
            let mut de =
                reikland::Deserializer::with_config(&data, DeserializerConfig::opinionated())
                    .context(DeserializeSnafu)?;
            SpeciesDexDeser {
                moves: &moves,
                items: &items,
                abilities: &abilities,
                types: &types,
                name_map: &name_map,
            }
            .deserialize(&mut de)
            .context(DeserializeSnafu)?
        };

        let map_names =
            MapNames::from_file(base.join("Data/MapInfos.rxdata")).context(DeserializeSnafu)?;
        // we collect encounters.dat and encounters_remix.dat and merge em together then with the
        // detected static and gift spawns
        let classic = Encounters::wild_rows(
            &read_dat(Path::new("Data/encounters.dat"))?,
            &species,
            &map_names,
        )
        .context(DeserializeSnafu)?;
        // Remix table is best-effort: Hoenn ships only an empty stub, so a read/parse miss just leaves every encounter Classic.
        let remix = read_dat(Path::new("Data/encounters_remix.dat"))
            .ok()
            .and_then(|bytes| Encounters::wild_rows(&bytes, &species, &map_names).ok())
            .map(|mut rows| {
                for row in &mut rows {
                    row.mode = EncounterMode::Remix;
                }
                rows
            })
            .unwrap_or_default();
        let mut encounter_rows = Encounters::merge_modes(classic, remix);
        let scrape =
            map_encounters::collect(&base.join("Data"), &species, &map_names, &items, &moves);
        let tm_locations = scrape.tm_locations;
        let tutor_locations = scrape.tutor_locations;
        let expert_locations = expert_moves::ExpertLocations {
            normal: scrape.expert_normal,
            legendary: scrape.expert_legendary,
        };
        encounter_rows.extend(scrape.encounters);
        encounter_rows.extend(settings_data::collect(
            &base.join("Data/Scripts"),
            &species,
            &map_names,
        ));
        let encounters = Encounters::from_rows(encounter_rows, species.len());
        let route_order = map_names.name_order();

        let stat_index = StatIndex::build(&species);
        let type_index = TypeFilterIndex::build(&species, &types);
        let ability_index = AbilityFilterIndex::build(&species, &abilities);
        let move_index = MoveFilterIndex::build(&species, &moves);
        let custom_sprite_index =
            CustomSpriteIndex::build(&species, &base.join("Data/sprites/CUSTOM_SPRITES"));

        let machine_moves = items
            .map()
            .values()
            .enumerate()
            .filter_map(|(i, item)| item.move_taught.map(|mv| (mv, ItemId::from_usize(i))))
            .collect();

        let expert_moves =
            expert_moves::collect(&base.join("Data/Scripts"), &moves, &species, &types);

        let legendaries = legendaries::collect(&base.join("Data/Scripts"), &species);
        let base_stats = species.map().values().map(|s| s.base_stats).collect();
        let dist = species.stat_distributions();
        let rank = dist.rank_table();
        let speed_curve = SpeedCurve::from_speed(&dist);

        Ok(Self {
            abilities,
            encounters,
            items,
            moves,
            species,
            types,
            stat_index,
            type_index,
            ability_index,
            move_index,
            custom_sprite_index,
            machine_moves,
            tm_locations,
            tutor_locations,
            expert_moves,
            expert_locations,
            route_order,
            legendaries,
            max_fusable_id: game_version.max_fusable_id(),
            base_stats,
            rank,
            speed_curve,
        })
    }

    pub fn types(&self) -> &TypeDex {
        &self.types
    }

    pub fn moves(&self) -> &MoveDex {
        &self.moves
    }

    pub fn abilities(&self) -> &AbilityDex {
        &self.abilities
    }

    pub fn items(&self) -> &ItemDex {
        &self.items
    }

    pub fn species(&self) -> &SpeciesDex {
        &self.species
    }

    pub fn encounters(&self) -> &Encounters {
        &self.encounters
    }

    pub fn stat_index(&self) -> &StatIndex {
        &self.stat_index
    }

    pub fn speed_curve(&self) -> SpeedCurve {
        self.speed_curve
    }

    /// The data the front end loads on open to build its controls (names + ids for every dex, plus the stat slider bounds)
    pub fn bootstrap(&self) -> Bootstrap {
        let min = self.species.min_stats();
        let max = self.species.max_stats();

        let species = self
            .species
            .map()
            .values()
            .enumerate()
            .map(|(i, s)| SpeciesOption {
                id: SpeciesId::from_usize(i),
                dex_id: s.id_number,
                name: s.name.clone(),
            })
            .collect();

        let mut types = named_ids(&self.types, |t| t.name.clone());
        types.retain(|t| t.name.as_ref() != "???");

        let (mut power_max, mut effect_max, mut accuracy_max) = (0u8, 0u8, 0u8);
        let (mut priority_min, mut priority_max) = (0i8, 0i8);
        for m in self.moves.map().values() {
            power_max = power_max.max(m.power.map_or(0, |p| p.get()));
            effect_max = effect_max.max(m.effect_chance.map_or(0, |p| p.get()));
            accuracy_max = accuracy_max.max(m.accuracy.percent().unwrap_or(0));
            priority_min = priority_min.min(m.priority);
            priority_max = priority_max.max(m.priority);
        }

        Bootstrap {
            species_count: self.species.len(),
            species,
            moves: self
                .moves
                .map()
                .values()
                .enumerate()
                .map(|(i, m)| MoveOption {
                    id: MoveId::from_usize(i),
                    name: m.name.clone(),
                    ty: m.ty,
                    category: m.category,
                    power: m.power,
                    effect_chance: m.effect_chance,
                    accuracy: m.accuracy,
                    priority: m.priority,
                    description: m.description.clone(),
                    flags: m.flags,
                })
                .collect(),
            types,
            abilities: named_ids(&self.abilities, |a| a.name.clone()),
            move_power: StatRange::new(0, power_max),
            move_effect_chance: StatRange::new(0, effect_max),
            move_accuracy: StatRange::new(0, accuracy_max),
            move_priority: StatRange::new(priority_min, priority_max),
            block_ids_above: self.max_fusable_id,
            stat_bounds: StatBounds {
                hp: StatRange::new(min.hp(), max.hp()),
                atk: StatRange::new(min.atk(), max.atk()),
                def: StatRange::new(min.def(), max.def()),
                spa: StatRange::new(min.spa(), max.spa()),
                spd: StatRange::new(min.spd(), max.spd()),
                spe: StatRange::new(min.spe(), max.spe()),
                bst: StatRange::new(self.species.min_bst(), self.species.max_bst()),
            },
        }
    }

    /// The display type ids of a fusion (1 for a mono-type fusion, else 2), given its encoded id
    pub fn fusion_type_ids(&self, fusion_id: u32) -> (TypeId, Option<TypeId>) {
        let n = self.species.len() as u32;
        let head = self
            .species
            .get_item(SpeciesId::from_usize((fusion_id / n) as usize));
        let body = self
            .species
            .get_item(SpeciesId::from_usize((fusion_id % n) as usize));
        fused_types(head, body, &self.types)
    }

    /// A fusion's name (head's first half + body's second half), given its encoded id
    pub fn fusion_name(&self, fusion_id: u32) -> inspect::FusionName {
        let n = self.species.len() as u32;
        let head = self
            .species
            .get_item(SpeciesId::from_usize((fusion_id / n) as usize));
        let body = self
            .species
            .get_item(SpeciesId::from_usize((fusion_id % n) as usize));
        inspect::FusionName {
            first_half: head.names.first_half.clone(),
            second_half: body.names.second_half.clone(),
        }
    }

    pub fn type_index(&self) -> &TypeFilterIndex {
        &self.type_index
    }

    pub fn ability_index(&self) -> &AbilityFilterIndex {
        &self.ability_index
    }

    pub fn move_index(&self) -> &MoveFilterIndex {
        &self.move_index
    }

    pub fn custom_sprite_index(&self) -> &CustomSpriteIndex {
        &self.custom_sprite_index
    }

    /// The TM/HM that teaches `move_id`, if any. Used to label machine moves in the move pool
    pub fn machine_for_move(&self, move_id: MoveId) -> Option<&str> {
        self.machine_moves
            .get(&move_id)
            .map(|&id| &*self.items.get_item(id).name)
    }

    /// Species indices flagged legendary by the game's `LEGENDARIES_LIST`
    pub fn legendaries(&self) -> &RoaringBitmap {
        &self.legendaries
    }
}

// hoenn XOR-"encrypts" its GameData `.dat` files with a key from `Data/Scripts/001_Technical/000_Encryption.rb`.
pub(crate) fn maybe_decrypt(mut bytes: Vec<u8>) -> Vec<u8> {
    // could grab with regex or reverse it in future?
    const KEY: [u8; 16] = [
        0x4A, 0x8F, 0x2C, 0xE1, 0x73, 0xB5, 0x96, 0x0D, 0x5E, 0xA2, 0x3F, 0xC7, 0x81, 0x14, 0x6B,
        0xD9,
    ];

    if !bytes.starts_with(&[0x04, 0x08]) {
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte ^= KEY[i % KEY.len()];
        }
    }

    bytes
}

pub trait DexId {
    fn from_usize(v: usize) -> Self;
    fn to_usize(self) -> usize;

    fn to_u32(self) -> u32;
    fn from_u32(v: u32) -> Self;
}

/// Immutable store of data for Pokemon Infinite Fusion. Any instance of [`Self::Id`] SHOULD always be valid.
pub trait Dex {
    /// Relative path from the root of InfiniteFusion to find the relevant file
    fn relative_path() -> &'static Path;

    type Id: DexId;
    type Item;

    fn map(&self) -> &IndexMap<Box<str>, Self::Item>;

    fn get(&self, id: Self::Id) -> (&str, &Self::Item) {
        self.map()
            .get_index(id.to_usize())
            .map(|(k, v)| (k.deref(), v))
            .expect("unmapped id")
    }

    fn get_item(&self, id: Self::Id) -> &Self::Item {
        self.map().get_index(id.to_usize()).map(|(_, v)| v).unwrap()
    }

    fn len(&self) -> usize {
        self.map().len()
    }

    fn is_empty(&self) -> bool {
        self.map().is_empty()
    }

    fn get_opt(&self, index: usize) -> Option<(&str, &Self::Item)> {
        self.map().get_index(index).map(|(k, v)| (k.deref(), v))
    }

    fn get_by_key(&self, key: &str) -> Option<&Self::Item> {
        self.map().get(key)
    }

    fn get_id_of(&self, key: &str) -> Option<Self::Id> {
        self.map()
            .get_full(key)
            .map(|(i, _, _)| Self::Id::from_usize(i))
    }

    fn get_full_by_key(&self, key: &str) -> Option<(Self::Id, &Self::Item)> {
        self.map()
            .get_full(key)
            .map(|(i, _, v)| (Self::Id::from_usize(i), v))
    }
}

impl<T> Dex for &T
where
    T: Dex,
{
    fn relative_path() -> &'static Path {
        T::relative_path()
    }

    type Id = T::Id;

    type Item = T::Item;

    fn map(&self) -> &IndexMap<Box<str>, Self::Item> {
        <T as Dex>::map(self)
    }

    fn get(&self, id: Self::Id) -> (&str, &Self::Item) {
        <T as Dex>::get(self, id)
    }

    fn get_item(&self, id: Self::Id) -> &Self::Item {
        <T as Dex>::get_item(self, id)
    }

    fn len(&self) -> usize {
        <T as Dex>::len(self)
    }

    fn is_empty(&self) -> bool {
        <T as Dex>::is_empty(self)
    }

    fn get_opt(&self, index: usize) -> Option<(&str, &Self::Item)> {
        <T as Dex>::get_opt(self, index)
    }

    fn get_by_key(&self, key: &str) -> Option<&Self::Item> {
        <T as Dex>::get_by_key(self, key)
    }

    fn get_id_of(&self, key: &str) -> Option<Self::Id> {
        <T as Dex>::get_id_of(self, key)
    }

    fn get_full_by_key(&self, key: &str) -> Option<(Self::Id, &Self::Item)> {
        <T as Dex>::get_full_by_key(self, key)
    }
}

#[derive(Debug)]
pub struct DexIdKeyVisitor<'a, T>(pub &'a T);

impl<T> Clone for DexIdKeyVisitor<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for DexIdKeyVisitor<'_, T> {}

impl<'de, 'a, T> DeserializeSeed<'de> for DexIdKeyVisitor<'a, T>
where
    T: Dex,
{
    type Value = T::Id;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        <&str as Deserialize>::deserialize(deserializer).and_then(|key| {
            self.0
                .get_id_of(key)
                .ok_or_else(|| serde::de::Error::custom(format_args!("{key} not found in dex")))
        })
    }
}

pub struct BoxCollector<S>(pub S);

impl<'de, S> DeserializeSeed<'de> for BoxCollector<S>
where
    S: DeserializeSeed<'de> + Copy,
{
    type Value = Box<[S::Value]>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(self)
    }
}

impl<'de, S> serde::de::Visitor<'de> for BoxCollector<S>
where
    S: DeserializeSeed<'de> + Copy,
{
    type Value = Box<[S::Value]>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a sequence")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let mut out = Vec::with_capacity(seq.size_hint().unwrap_or(0));
        while let Some(elem) = seq.next_element_seed(self.0)? {
            out.push(elem);
        }
        Ok(out.into_boxed_slice())
    }
}
