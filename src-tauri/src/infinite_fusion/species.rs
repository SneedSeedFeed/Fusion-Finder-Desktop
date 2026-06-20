use std::path::Path;

use indexmap::IndexMap;
use itertools::Itertools;
use reikland::MixedKeyRef;
use serde::{
    Deserialize, Serialize,
    de::{DeserializeSeed, IgnoredAny, Visitor},
};
use snafu::{OptionExt, Snafu};

use crate::{
    dex_id,
    infinite_fusion::{
        BoxCollector, Dex, DexId, DexIdKeyVisitor,
        abilities::{AbilityDex, AbilityId},
        items::ItemDex,
        moves::{MoveDex, MoveId},
        species::{
            base_stats::{BaseStats, StatDistributions},
            evolution::{Evolution, EvolutionVisitor, UnmappedEvolution},
            level_move::{LevelMove, LevelMoveVisitor},
            name_halves::{NameHalves, NameMap},
        },
        types::{TypeDex, TypeId},
    },
};

pub(crate) mod base_stats;
pub(crate) mod evolution;
pub(crate) mod level_move;
pub(crate) mod name_halves;

#[derive(Debug, Clone)]
pub struct SpeciesDex {
    map: IndexMap<Box<str>, SpeciesDetails>,
    min_stats: BaseStats,
    max_stats: BaseStats,
    min_bst: u16,
    max_bst: u16,
    stat_distributions: StatDistributions,
}

impl Dex for SpeciesDex {
    fn relative_path() -> &'static Path {
        Path::new("Data/species.dat")
    }

    type Id = SpeciesId;

    type Item = SpeciesDetails;

    fn map(&self) -> &IndexMap<Box<str>, Self::Item> {
        &self.map
    }
}

impl SpeciesDex {
    pub fn min_stats(&self) -> &BaseStats {
        &self.min_stats
    }

    pub fn max_stats(&self) -> &BaseStats {
        &self.max_stats
    }

    pub fn stat_distributions(&self) -> &StatDistributions {
        &self.stat_distributions
    }

    pub fn min_bst(&self) -> u16 {
        self.min_bst
    }

    pub fn max_bst(&self) -> u16 {
        self.max_bst
    }
}

dex_id!(SpeciesId, u16);

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct SpeciesDetails {
    pub id_number: u16,
    pub name: Box<str>,
    pub names: NameHalves,
    pub type1: TypeId,
    pub type2: Option<TypeId>,
    pub base_stats: BaseStats,
    pub moves: Box<[LevelMove]>,
    pub tutor_moves: Box<[MoveId]>,
    pub egg_moves: Box<[MoveId]>,
    pub abilities: Box<[AbilityId]>,
    pub hidden_abilities: Box<[AbilityId]>,
    pub evolutions: Box<[Evolution]>,
}

/// Errors when resolving species details
#[derive(Debug, Snafu)]
pub enum SpeciesMapNotFoundError {
    #[snafu(display("type {type_name:?} not found in the TypeDex"))]
    Type { type_name: Box<str> },

    #[snafu(display("evolution target {target:?} not found in the species dex"))]
    EvoTarget { target: Box<str> },

    #[snafu(display("fusion dex number {dex_number} not found in the NameMap"))]
    Name { dex_number: u16 },
}

pub struct UnmappedSpeciesDetails<'a> {
    id_number: u32, // triple fusions push this up to u32 until we filter them out
    name: &'a str,
    type1: &'a str,
    type2: Option<&'a str>,
    base_stats: BaseStats,
    moves: Box<[LevelMove]>,
    tutor_moves: Box<[MoveId]>,
    egg_moves: Box<[MoveId]>,
    abilities: Box<[AbilityId]>,
    hidden_abilities: Box<[AbilityId]>,
    evolutions: Box<[UnmappedEvolution<'a>]>,
}

impl UnmappedSpeciesDetails<'_> {
    fn map(
        dex: IndexMap<Box<str>, Self>,
        types: &TypeDex,
        name_map: &NameMap,
    ) -> Result<IndexMap<Box<str>, SpeciesDetails>, SpeciesMapNotFoundError> {
        let all_evos = dex
            .values()
            .map(|v| v.map_evos(&dex))
            .collect::<Result<Vec<_>, _>>()?;

        dex.into_iter()
            .zip_eq(all_evos)
            .map(|((key, v), evos)| {
                let details = v.assign_evos(evos, types, name_map, &key)?;
                Ok((key, details))
            })
            .collect()
    }

    /// Form species share their base species' fusion name but carry no fusion-dex mapping of their own, so they're resolved by the base species' national dex number instead.
    fn name_dex_override(symbol: &str) -> Option<u16> {
        Some(match symbol {
            "SHELLOS_E" | "SHELLOS_W" => 422,     // Shellos
            "GASTRODON_E" | "GASTRODON_W" => 423, // Gastrodon
            _ => return None,
        })
    }

    fn map_evos(
        &self,
        dex: &IndexMap<Box<str>, Self>,
    ) -> Result<Box<[Evolution]>, SpeciesMapNotFoundError> {
        self.evolutions
            .iter()
            .map(|evo| {
                let target = evo.target();
                let (idx, _, _) = dex.get_full(target).context(EvoTargetSnafu { target })?;
                Ok(evo.assign_id(SpeciesId::from_usize(idx)))
            })
            .collect()
    }

    fn assign_evos(
        self,
        evolutions: Box<[Evolution]>,
        types: &TypeDex,
        name_map: &NameMap,
        symbol: &str,
    ) -> Result<SpeciesDetails, SpeciesMapNotFoundError> {
        let id_number = self.id_number as u16;
        let names = match Self::name_dex_override(symbol) {
            Some(national_dex) => name_map.get_by_national_dex(national_dex),
            None => name_map.get_name_halves(id_number),
        }
        .context(NameSnafu {
            dex_number: id_number,
        })?
        .clone();

        let type1 = types.get_id_of(self.type1).context(TypeSnafu {
            type_name: self.type1,
        })?;

        // turn monotypes into Type1 / None instead of Type1 / Type1
        let type2 = self
            .type2
            .map(|type2| {
                types
                    .get_id_of(type2)
                    .context(TypeSnafu { type_name: type2 })
            })
            .transpose()?
            .filter(|&type2| type2 != type1);

        Ok(SpeciesDetails {
            id_number,
            name: self.name.into(),
            names,
            type1,
            type2,
            base_stats: self.base_stats,
            moves: self.moves,
            tutor_moves: self.tutor_moves,
            egg_moves: self.egg_moves,
            abilities: self.abilities,
            hidden_abilities: self.hidden_abilities,
            evolutions,
        })
    }
}

pub struct SpeciesDexDeser<'a> {
    pub moves: &'a MoveDex,
    pub items: &'a ItemDex,
    pub abilities: &'a AbilityDex,
    pub types: &'a TypeDex,
    pub name_map: &'a NameMap,
}

impl<'a, 'de> DeserializeSeed<'de> for SpeciesDexDeser<'a> {
    type Value = SpeciesDex;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(self)
    }
}

impl<'a, 'de> Visitor<'de> for SpeciesDexDeser<'a> {
    type Value = SpeciesDex;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("species.dat (a ruby hash)")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut unmapped = IndexMap::new();

        let mut min_stats = BaseStats::MAX;
        let mut max_stats = BaseStats::MIN;
        let mut min_bst = u16::MAX;
        let mut max_bst = u16::MIN;
        let mut stat_distributions = StatDistributions::default();

        while let Some(key) = map.next_key::<MixedKeyRef>()? {
            match key {
                MixedKeyRef::Int(_) => {
                    map.next_value::<IgnoredAny>()?;
                }
                MixedKeyRef::Str(sym) => {
                    if let Some(details) = map.next_value_seed(UnmappedSpeciesDetailsDeser {
                        moves: self.moves,
                        items: self.items,
                        abilities: self.abilities,
                    })? {
                        min_stats.apply_min(&details.base_stats);
                        max_stats.apply_max(&details.base_stats);
                        stat_distributions.record(&details.base_stats);
                        let bst = details.base_stats.bst();
                        min_bst = min_bst.min(bst);
                        max_bst = max_bst.max(bst);
                        unmapped.insert(Box::from(sym), details);
                    }
                }
            }
        }

        let map = UnmappedSpeciesDetails::map(unmapped, self.types, self.name_map)
            .map_err(serde::de::Error::custom)?;

        Ok(SpeciesDex {
            map,
            min_bst,
            max_bst,
            max_stats,
            min_stats,
            stat_distributions,
        })
    }
}

struct UnmappedSpeciesDetailsDeser<'a> {
    moves: &'a MoveDex,
    items: &'a ItemDex,
    abilities: &'a AbilityDex,
}

impl<'a, 'de> DeserializeSeed<'de> for UnmappedSpeciesDetailsDeser<'a> {
    type Value = Option<UnmappedSpeciesDetails<'de>>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(self)
    }
}

impl<'a, 'de> Visitor<'de> for UnmappedSpeciesDetailsDeser<'a> {
    type Value = Option<UnmappedSpeciesDetails<'de>>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a species.dat entry")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut id_number = None;
        let mut name = None;
        let mut type1 = None;
        let mut type2 = None;
        let mut base_stats = None;
        let mut moves = None;
        let mut tutor_moves = None;
        let mut egg_moves = None;
        let mut abilities = None;
        let mut hidden_abilities = None;
        let mut evolutions = None;

        // have we already confirmed we can throw this species' details away?
        let mut discard = false;

        while let Some(key) = map.next_key::<&str>()? {
            if discard {
                map.next_value::<IgnoredAny>()?;
                continue;
            }

            match key {
                "@id_number" => {
                    let n = map.next_value::<u32>()?;
                    // Discard triple fusions: their dex numbers overflow u16 and we don't model them.
                    discard = n >= u32::from(u16::MAX);
                    id_number = Some(n);
                }
                "@real_name" => name = Some(map.next_value::<&str>()?),
                "@type1" => type1 = Some(map.next_value::<&str>()?),
                "@type2" => type2 = Some(map.next_value::<&str>()?),
                "@base_stats" => base_stats = Some(map.next_value::<BaseStats>()?),
                "@moves" => {
                    moves = Some(map.next_value_seed(BoxCollector(LevelMoveVisitor(self.moves)))?)
                }
                "@tutor_moves" => {
                    tutor_moves =
                        Some(map.next_value_seed(BoxCollector(DexIdKeyVisitor(self.moves)))?)
                }
                "@egg_moves" => {
                    egg_moves =
                        Some(map.next_value_seed(BoxCollector(DexIdKeyVisitor(self.moves)))?)
                }
                "@abilities" => {
                    abilities =
                        Some(map.next_value_seed(BoxCollector(DexIdKeyVisitor(self.abilities)))?)
                }
                "@hidden_abilities" => {
                    hidden_abilities =
                        Some(map.next_value_seed(BoxCollector(DexIdKeyVisitor(self.abilities)))?)
                }
                "@evolutions" => {
                    evolutions = Some(map.next_value_seed(BoxCollector(EvolutionVisitor {
                        move_dex: self.moves,
                        item_dex: self.items,
                    }))?)
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }

        if discard {
            return Ok(None);
        }

        let missing_field =
            |field: &'static str| <A::Error as serde::de::Error>::missing_field(field);

        Ok(Some(UnmappedSpeciesDetails {
            id_number: id_number.ok_or_else(|| missing_field("@id_number"))?,
            name: name.ok_or_else(|| missing_field("@real_name"))?,
            type1: type1.ok_or_else(|| missing_field("@type1"))?,
            type2,
            base_stats: base_stats.ok_or_else(|| missing_field("@base_stats"))?,
            moves: moves.ok_or_else(|| missing_field("@moves"))?,
            tutor_moves: tutor_moves.ok_or_else(|| missing_field("@tutor_moves"))?,
            egg_moves: egg_moves.ok_or_else(|| missing_field("@egg_moves"))?,
            abilities: abilities.ok_or_else(|| missing_field("@abilities"))?,
            hidden_abilities: hidden_abilities.ok_or_else(|| missing_field("@hidden_abilities"))?,
            evolutions: evolutions.ok_or_else(|| missing_field("@evolutions"))?,
        }))
    }
}

#[cfg(test)]
pub(crate) mod test {
    use reikland::DeserializerConfig;
    use serde::de::DeserializeSeed;

    use crate::{
        infinite_fusion::{
            Dex,
            abilities::test::load_abilities,
            items::test::load_items,
            moves::test::load_moves,
            species::{SpeciesDex, SpeciesDexDeser},
            types::test::load_types,
        },
        test::{infinite_fusion_dir, infinite_fusion_hoenn_dir, maybe_decrypt},
    };

    use super::name_halves::NameMap;

    /// `[classic, hoenn]`
    pub(crate) fn load_species() -> [SpeciesDex; 2] {
        let dirs = [infinite_fusion_dir(), infinite_fusion_hoenn_dir()];
        // SplitNames.rb sits at a different relative path in Hoenn (extra `Data/` segment).
        let name_paths = [
            infinite_fusion_dir().join(NameMap::relative_path()),
            infinite_fusion_hoenn_dir().join(NameMap::relative_path_hoenn()),
        ];

        let types = load_types();
        let moves = load_moves();
        let abilities = load_abilities();
        let items = load_items();

        std::array::from_fn(|i| {
            let name_map = NameMap::from_file(&name_paths[i]).unwrap();

            let data =
                maybe_decrypt(std::fs::read(dirs[i].join(SpeciesDex::relative_path())).unwrap());
            let mut deser =
                reikland::Deserializer::with_config(&data, DeserializerConfig::opinionated())
                    .unwrap();

            SpeciesDexDeser {
                moves: &moves[i],
                items: &items[i],
                abilities: &abilities[i],
                types: &types[i],
                name_map: &name_map,
            }
            .deserialize(&mut deser)
            .unwrap()
        })
    }

    #[test]
    fn deser_species_dat() {
        let [classic, hoenn] = load_species();

        for species in [&classic, &hoenn] {
            assert!(!species.is_empty());

            // real bounds, not the extremes the accumulators start from (a swapped
            // initialisation would leave them pinned at MIN/MAX)
            assert!(species.min_bst > u16::MIN && species.max_bst < u16::MAX);
            assert!(species.min_bst < species.max_bst);
            assert_ne!(*species.min_stats(), super::base_stats::BaseStats::MIN);
            assert_ne!(*species.max_stats(), super::base_stats::BaseStats::MAX);

            let bulbasaur = species.get_by_key("BULBASAUR").expect("BULBASAUR exists");
            assert_eq!(&*bulbasaur.name, "Bulbasaur");
            assert!(bulbasaur.type2.is_some());
            assert!(!bulbasaur.moves.is_empty());
            assert_eq!(bulbasaur.evolutions.len(), 1);
            assert!(species.get_id_of("IVYSAUR").is_some());

            // best starter ever btw
            let turtwig = species.get_by_key("TURTWIG").expect("TURTWIG exists");
            assert!(turtwig.type2.is_none()); // turtwig should be GRASS / NONE not GRASS / GRASS after our filtering
        }

        // shellos line introduced in hoenn don't have separate name entries but have separate IDs so we have to hardcode for them (for now)
        let shellos_e = hoenn
            .get_by_key("SHELLOS_E")
            .expect("SHELLOS_E exists in Hoenn");
        assert_eq!(&*shellos_e.names.first_half, "Shell");
        assert_eq!(&*shellos_e.names.second_half, "los");
    }
}
