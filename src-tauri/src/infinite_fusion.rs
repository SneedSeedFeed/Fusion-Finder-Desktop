use std::{
    ops::Deref,
    path::{Path, PathBuf},
};

use indexmap::IndexMap;
use reikland::DeserializerConfig;
use serde::{Deserialize, Serialize, de::DeserializeSeed};
use snafu::{ResultExt, Snafu};

use crate::infinite_fusion::{
    abilities::AbilityDex,
    encounters::{Encounters, MapNames},
    filters::{
        FilterOptions, SpeciesOption, StatBounds, StatRange, ability_filter::AbilityFilterIndex,
        custom_sprite_filter::CustomSpriteIndex, move_filter::MoveFilterIndex, named_ids,
        stat_filter::StatIndex, type_filter::TypeFilterIndex,
    },
    items::{ItemDex, ItemDexDeser},
    moves::{MoveDex, MoveDexDeser},
    species::{SpeciesDex, SpeciesDexDeser, SpeciesId, name_halves::NameMap},
    types::TypeDex,
};

pub mod abilities;
pub mod encounters;
pub mod filters;
pub mod items;
pub mod moves;
pub mod species;
pub mod types;

// maybe just Box::leak this whole thing since the core data is immutable and it's going to get shared between threads??
/// All data across every fusion. big old type since it's multiple indexmaps so ideally get some pointer around it
#[derive(Debug, Clone)]
pub struct InfiniteFusionDex {
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
    /// highest in-game dex number this game can actually fuse (`None` = no cap); feeds the hidden
    /// `block_ids_above` filter
    max_fusable_id: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub struct FusionId {
    pub head: SpeciesId,
    pub body: SpeciesId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, strum::EnumString)]
pub enum GameVersion {
    Kanto,
    Hoenn,
}

impl GameVersion {
    // kanto species.dat contains species not actually in Kanto
    /// Highest in-game dex number actually fusable in this game
    pub fn max_fusable_id(self) -> Option<u16> {
        match self {
            GameVersion::Kanto => Some(501),
            GameVersion::Hoenn => None,
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
        let encounters = Encounters::from_bytes(
            &read_dat(Path::new("Data/encounters.dat"))?,
            &species,
            &map_names,
        )
        .context(DeserializeSnafu)?;

        let stat_index = StatIndex::build(&species);
        let type_index = TypeFilterIndex::build(&species, &types);
        let ability_index = AbilityFilterIndex::build(&species, &abilities);
        let move_index = MoveFilterIndex::build(&species, &moves);
        let custom_sprite_index =
            CustomSpriteIndex::build(&species, &base.join("Data/sprites/CUSTOM_SPRITES"));

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
            max_fusable_id: game_version.max_fusable_id(),
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

    /// The data the front end loads on open to build its filter controls (names + ids for every dex, plus the stat slider bounds).
    pub fn filter_options(&self) -> FilterOptions {
        let min = self.species.min_stats();
        let max = self.species.max_stats();

        let species = self
            .species
            .map()
            .values()
            .enumerate()
            .map(|(i, s)| SpeciesOption {
                id: SpeciesId::from_usize(i).to_u32(),
                dex_id: s.id_number,
                name: s.name.to_string(),
                first: s.names.first_half.to_string(),
                second: s.names.second_half.to_string(),
            })
            .collect();

        let mut types = named_ids(&self.types, |t| t.name.to_string());
        types.retain(|t| t.name != "???");

        FilterOptions {
            species_count: self.species.len(),
            species,
            moves: named_ids(&self.moves, |m| m.name.to_string()),
            types,
            abilities: named_ids(&self.abilities, |a| a.name.to_string()),
            block_ids_above: self.max_fusable_id,
            stat_bounds: StatBounds {
                hp: StatRange { min: min.hp(), max: max.hp() },
                atk: StatRange { min: min.atk(), max: max.atk() },
                def: StatRange { min: min.def(), max: max.def() },
                spa: StatRange { min: min.spa(), max: max.spa() },
                spd: StatRange { min: min.spd(), max: max.spd() },
                spe: StatRange { min: min.spe(), max: max.spe() },
                bst: StatRange { min: self.species.min_bst(), max: self.species.max_bst() },
            },
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
