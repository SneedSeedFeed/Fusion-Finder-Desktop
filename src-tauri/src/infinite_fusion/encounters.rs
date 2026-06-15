use std::{collections::HashMap, ops::Range, path::Path, sync::Arc};

use reikland::{DeserializerConfig, MixedKeyRef};
use serde::{
    Deserialize, Serialize,
    de::{DeserializeSeed, IgnoredAny, Visitor},
};
use strum::{EnumString, VariantNames};

use crate::infinite_fusion::{
    BoxCollector, Dex, DexId, DexIdKeyVisitor,
    species::{SpeciesDex, SpeciesId},
};

#[derive(Debug, Clone)]
pub struct Encounters {
    rows: Box<[Encounter]>,
    /// contiguous slice of [`Self::rows`] for a given [`SpeciesId`]
    by_species: Box<[Range<u16>]>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Encounter {
    pub species: SpeciesId,
    pub route: Arc<str>, // arc so we don't duplicate it per row (also need arc for tauri)
    pub method: EncounterMethod,
    /// % chance that an encounter on this route is this species.
    pub chance: u8,
    pub min_level: u8,
    pub max_level: u8,
    pub mode: EncounterMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
pub enum EncounterMode {
    Classic,
    Remix,
    Both,
}

impl Encounters {
    /// Every place the given species can be encountered in the wild.
    pub fn for_species(&self, species: SpeciesId) -> &[Encounter] {
        match self.by_species.get(species.to_usize()) {
            Some(range) => &self.rows[range.start as usize..range.end as usize],
            None => &[],
        }
    }

    pub fn all(&self) -> &[Encounter] {
        &self.rows
    }

    pub fn from_bytes(
        bytes: &[u8],
        species: &SpeciesDex,
        map_names: &MapNames,
    ) -> Result<Self, reikland::MarshalDeserializeError> {
        Ok(Self::from_rows(
            Self::wild_rows(bytes, species, map_names)?,
            species.len(),
        ))
    }

    /// Wild encounters pulled from game data
    pub(crate) fn wild_rows(
        bytes: &[u8],
        species: &SpeciesDex,
        map_names: &MapNames,
    ) -> Result<Vec<Encounter>, reikland::MarshalDeserializeError> {
        let mut deser =
            reikland::Deserializer::with_config(bytes, DeserializerConfig::opinionated())?;
        EncountersDeser { species, map_names }.deserialize(&mut deser)
    }

    /// Merge mode data and collapse identical encounters between modes
    pub(crate) fn merge_modes(classic: Vec<Encounter>, remix: Vec<Encounter>) -> Vec<Encounter> {
        // everything that identifies an encounter *except* its mode
        type Key = (SpeciesId, Arc<str>, EncounterMethod, u8, u8, u8);
        let key = |e: &Encounter| {
            (
                e.species,
                e.route.clone(),
                e.method,
                e.chance,
                e.min_level,
                e.max_level,
            )
        };

        let mut seen: HashMap<Key, EncounterMode> = HashMap::new();
        for e in &classic {
            seen.insert(key(e), e.mode);
        }
        for e in &remix {
            seen.entry(key(e))
                .and_modify(|m| *m = EncounterMode::Both)
                .or_insert(e.mode);
        }

        seen.into_iter()
            .map(
                |((species, route, method, chance, min_level, max_level), mode)| Encounter {
                    species,
                    route,
                    method,
                    chance,
                    min_level,
                    max_level,
                    mode,
                },
            )
            .collect()
    }

    /// Build the by-species index over `rows`. `species_count` sizes the lookup table.
    pub(crate) fn from_rows(mut rows: Vec<Encounter>, species_count: usize) -> Self {
        // group rows by species so `for_species` is a slice into a single allocation.
        rows.sort_by_key(|e| e.species);

        let mut by_species = vec![0..0; species_count].into_boxed_slice();
        let mut i = 0;
        while i < rows.len() {
            let species = rows[i].species.to_usize();
            let start = i;
            while i < rows.len() && rows[i].species.to_usize() == species {
                i += 1;
            }
            by_species[species] = start as u16..i as u16;
        }

        Encounters {
            rows: rows.into_boxed_slice(),
            by_species,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, VariantNames, EnumString)]
pub enum EncounterMethod {
    Land,
    Land1,
    Land2,
    Land3,
    LandDay,
    LandMorning,
    LandNight,
    LandFog,
    LandRain,
    LandStorm,
    LandSunny,
    LandWind,
    TallGrass,
    Cave,
    Water,
    WaterNight,
    WaterFog,
    WaterRain,
    WaterStorm,
    WaterSunny,
    WaterWind,
    OldRod,
    GoodRod,
    SuperRod,
    RockSmash,
    /// Scripted fixed encounter recovered from map event scripts.
    Static,
    /// Pokemon handed to the player by an event (starters, fossils, in-game gifts).
    Gift,
    Roaming,
    /// A wild encounter only reachable with the Radar
    PokeRadar,
}

impl<'de> Deserialize<'de> for EncounterMethod {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Vis;
        impl Visitor<'_> for Vis {
            type Value = EncounterMethod;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("an encounter method symbol")
            }

            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                s.parse()
                    .map_err(|_| E::unknown_variant(s, EncounterMethod::VARIANTS))
            }
        }

        deserializer.deserialize_str(Vis)
    }
}

#[derive(Debug, Clone)]
pub struct MapNames(HashMap<u16, Arc<str>>);

impl MapNames {
    pub fn get(&self, map_id: u16) -> Option<&Arc<str>> {
        self.0.get(&map_id)
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, reikland::MarshalDeserializeError> {
        let bytes = std::fs::read(path).expect("failed to read MapInfos.rxdata");
        reikland::from_bytes_with_config::<MapNames>(&bytes, DeserializerConfig::opinionated())
    }
}

impl<'de> Deserialize<'de> for MapNames {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Vis;
        impl<'de> Visitor<'de> for Vis {
            type Value = MapNames;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("MapInfos.rxdata (a ruby hash of RPG::MapInfo)")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                #[derive(Deserialize)]
                struct MapInfo {
                    #[serde(rename = "@name")]
                    name: Box<str>,
                }

                let mut names = HashMap::new();
                while let Some(key) = map.next_key::<MixedKeyRef>()? {
                    match key {
                        MixedKeyRef::Int(id) => {
                            let info = map.next_value::<MapInfo>()?;
                            names.insert(id as u16, Arc::from(info.name));
                        }
                        MixedKeyRef::Str(_) => {
                            map.next_value::<IgnoredAny>()?;
                        }
                    }
                }
                names.shrink_to_fit();
                Ok(MapNames(names))
            }
        }

        deserializer.deserialize_map(Vis)
    }
}

struct EncountersDeser<'a> {
    species: &'a SpeciesDex,
    map_names: &'a MapNames,
}

impl<'a, 'de> DeserializeSeed<'de> for EncountersDeser<'a> {
    type Value = Vec<Encounter>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(self)
    }
}

impl<'a, 'de> Visitor<'de> for EncountersDeser<'a> {
    type Value = Vec<Encounter>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("encounters.dat (a ruby hash)")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut rows: Vec<Encounter> = Vec::new();

        while let Some(key) = map.next_key::<MixedKeyRef>()? {
            match key {
                MixedKeyRef::Int(_) => {
                    map.next_value::<IgnoredAny>()?;
                }
                MixedKeyRef::Str(_) => {
                    let entry = map.next_value_seed(EncounterEntryDeser {
                        species: self.species,
                        map_names: self.map_names,
                    })?;
                    rows.extend(entry);
                }
            }
        }

        Ok(rows)
    }
}

struct EncounterEntryDeser<'a> {
    species: &'a SpeciesDex,
    map_names: &'a MapNames,
}

impl<'a, 'de> DeserializeSeed<'de> for EncounterEntryDeser<'a> {
    type Value = Vec<Encounter>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(self)
    }
}

impl<'a, 'de> Visitor<'de> for EncounterEntryDeser<'a> {
    type Value = Vec<Encounter>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("an encounters.dat entry")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut map_id = None;
        let mut tables = None;

        while let Some(key) = map.next_key::<&str>()? {
            match key {
                "@map" => map_id = Some(map.next_value::<i32>()? as u16),
                "@types" => tables = Some(map.next_value_seed(MethodTablesDeser(self.species))?),
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }

        let map_id = map_id.ok_or_else(|| serde::de::Error::missing_field("@map"))?;
        let tables = tables.ok_or_else(|| serde::de::Error::missing_field("@types"))?;

        // un-named maps also lack map files so we can skip safely
        let Some(route) = self.map_names.get(map_id).cloned() else {
            return Ok(Vec::new());
        };

        let rows = tables
            .into_iter()
            .flat_map(|(method, slots)| {
                let route = route.clone();
                slots.into_vec().into_iter().map(move |slot| Encounter {
                    species: slot.species,
                    route: route.clone(),
                    method,
                    chance: slot.weight,
                    min_level: slot.min_level,
                    max_level: slot.max_level,
                    mode: EncounterMode::Classic,
                })
            })
            .collect();

        Ok(rows)
    }
}

struct Slot {
    weight: u8,
    species: SpeciesId,
    min_level: u8,
    max_level: u8,
}

struct MethodTablesDeser<'a>(&'a SpeciesDex);

impl<'a, 'de> DeserializeSeed<'de> for MethodTablesDeser<'a> {
    type Value = Vec<(EncounterMethod, Box<[Slot]>)>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(self)
    }
}

impl<'a, 'de> Visitor<'de> for MethodTablesDeser<'a> {
    type Value = Vec<(EncounterMethod, Box<[Slot]>)>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a hash of encounter method to slots")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut tables = Vec::new();
        while let Some(method) = map.next_key::<EncounterMethod>()? {
            let slots = map.next_value_seed(BoxCollector(SlotDeser(self.0)))?;
            tables.push((method, slots));
        }
        Ok(tables)
    }
}

#[derive(Clone, Copy)]
struct SlotDeser<'a>(&'a SpeciesDex);

impl<'a, 'de> DeserializeSeed<'de> for SlotDeser<'a> {
    type Value = Slot;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(self)
    }
}

impl<'a, 'de> Visitor<'de> for SlotDeser<'a> {
    type Value = Slot;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("[weight, species, min_level, max_level]")
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let weight = seq
            .next_element::<u8>()?
            .ok_or_else(|| serde::de::Error::custom("missing encounter slot weight"))?;
        let species = seq
            .next_element_seed(DexIdKeyVisitor(self.0))?
            .ok_or_else(|| serde::de::Error::custom("missing encounter slot species"))?;
        let min_level = seq
            .next_element::<u8>()?
            .ok_or_else(|| serde::de::Error::custom("missing encounter slot min level"))?;
        let max_level = seq
            .next_element::<u8>()?
            .ok_or_else(|| serde::de::Error::custom("missing encounter slot max level"))?;

        Ok(Slot {
            weight,
            species,
            min_level,
            max_level,
        })
    }
}

#[cfg(test)]
pub(crate) mod test {
    use crate::{
        infinite_fusion::encounters::{EncounterMethod, EncounterMode, Encounters, MapNames},
        test::{infinite_fusion_dir, infinite_fusion_hoenn_dir, maybe_decrypt},
    };

    /// `[classic, hoenn]`
    pub(crate) fn load_encounters() -> [Encounters; 2] {
        let dirs = [infinite_fusion_dir(), infinite_fusion_hoenn_dir()];
        let species = crate::infinite_fusion::species::test::load_species();

        std::array::from_fn(|i| {
            let map_names = MapNames::from_file(dirs[i].join("Data/MapInfos.rxdata")).unwrap();
            let bytes = maybe_decrypt(std::fs::read(dirs[i].join("Data/encounters.dat")).unwrap());

            Encounters::from_bytes(&bytes, &species[i], &map_names).unwrap()
        })
    }

    #[test]
    fn deser_encounters_dat() {
        let [classic, hoenn] = load_encounters();

        for encounters in [&classic, &hoenn] {
            assert!(!encounters.all().is_empty());

            // the by-species index returns a coherent slice for an arbitrary row's species
            let first = &encounters.all()[0];
            let slice = encounters.for_species(first.species);
            assert!(!slice.is_empty());
            assert!(slice.iter().all(|e| e.species == first.species));
        }

        let abra_route24 = classic
            .all()
            .iter()
            .find(|e| {
                &*e.route == "Route 24" && e.method == EncounterMethod::Land && e.chance == 20
            })
            .expect("Abra should be a Route 24 grass encounter");
        assert_eq!((abra_route24.min_level, abra_route24.max_level), (8, 12));
    }

    #[test]
    fn merges_classic_and_remix_modes() {
        let dir = infinite_fusion_dir();
        let species = crate::infinite_fusion::species::test::load_species();
        let map_names = MapNames::from_file(dir.join("Data/MapInfos.rxdata")).unwrap();

        let classic = Encounters::wild_rows(
            &maybe_decrypt(std::fs::read(dir.join("Data/encounters.dat")).unwrap()),
            &species[0],
            &map_names,
        )
        .unwrap();
        let mut remix = Encounters::wild_rows(
            &maybe_decrypt(std::fs::read(dir.join("Data/encounters_remix.dat")).unwrap()),
            &species[0],
            &map_names,
        )
        .unwrap();
        for row in &mut remix {
            row.mode = EncounterMode::Remix;
        }

        let merged = Encounters::merge_modes(classic.clone(), remix.clone());

        // merging dedups shared rows, so the result is smaller than the naive concatenation
        assert!(merged.len() < classic.len() + remix.len());
        // and every mode is represented: shared encounters, plus ones exclusive to each table
        assert!(merged.iter().any(|e| e.mode == EncounterMode::Both));
        assert!(merged.iter().any(|e| e.mode == EncounterMode::Classic));
        assert!(merged.iter().any(|e| e.mode == EncounterMode::Remix));
    }
}
