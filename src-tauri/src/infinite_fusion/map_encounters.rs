//! Static / gift encounters recovered from map event scripts.
//!
//! We deserialize the maps, pull the script-command lines (RGSS event codes 355 = "Script", 655 = continuation) and scrape
//! `pbWildBattle` / `pbAddPokemon` / `pbCreatePokemon` calls back out into [`Encounter`] rows.

use std::{collections::HashSet, path::Path, sync::LazyLock};

use regex::Regex;
use reikland::DeserializerConfig;
use serde::{
    Deserialize,
    de::{IgnoredAny, Visitor},
};

use crate::infinite_fusion::{
    Dex,
    encounters::{Encounter, EncounterMethod, EncounterMode, MapNames},
    species::{SpeciesDex, SpeciesId},
};

/// Captures `pbWildBattle(:SPECIES, level…` and friends.
/// The species symbol and the first numeric argument (the level) are the two groups we keep.
static ENCOUNTER_CALL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(pbWildBattle|pbCreatePokemon|pbAddPokemon|pbAddPokemonSilent)\(\s*:([A-Z][A-Z0-9_]*)\s*,\s*(\d+)",
    )
    .unwrap()
});

/// Scrape every `Map###.rxdata` in `data_dir` for scripted encounters. Failures are skipped as this is just meant to supplement the main encounters (and could be spotty)
pub fn collect(data_dir: &Path, species: &SpeciesDex, map_names: &MapNames) -> Vec<Encounter> {
    let mut rows = Vec::new();
    // de-dup identical (species, route, method, level) hits — event pages repeat their command list.
    let mut seen: HashSet<(SpeciesId, u16, EncounterMethod, u8)> = HashSet::new();

    let Ok(dir) = std::fs::read_dir(data_dir) else {
        return rows;
    };

    for entry in dir.flatten() {
        let path = entry.path();
        let Some(map_id) = map_id_from_path(&path) else {
            continue;
        };
        // un-named maps also lack a friendly route name, so skip them as the wild table does.
        let Some(route) = map_names.get(map_id).cloned() else {
            continue;
        };
        let Ok(bytes) = std::fs::read(&path) else {
            continue;
        };
        let Ok(map) =
            reikland::from_bytes_with_config::<RpgMap>(&bytes, DeserializerConfig::opinionated())
        else {
            continue;
        };

        for line in map.script_lines() {
            for caps in ENCOUNTER_CALL.captures_iter(line) {
                let func = &caps[1];
                let Some(species_id) = species.get_id_of(&caps[2]) else {
                    continue; // computed fusion ids (e.g. "B124H80") aren't species.dat entries
                };
                let Ok(level) = caps[3].parse::<u8>() else {
                    continue;
                };
                let method = match func {
                    "pbAddPokemon" | "pbAddPokemonSilent" => EncounterMethod::Gift,
                    _ => EncounterMethod::Static,
                };
                if seen.insert((species_id, map_id, method, level)) {
                    rows.push(Encounter {
                        species: species_id,
                        route: route.clone(),
                        method,
                        chance: 100,
                        min_level: level,
                        max_level: level,
                        mode: EncounterMode::Both, // scripted events fire regardless of game mode
                    });
                }
            }
        }
    }

    rows
}

/// `…/Map006.rxdata` -> `6`. Returns `None` for anything that isn't a numbered map file
/// (e.g. `MapInfos.rxdata`).
fn map_id_from_path(path: &Path) -> Option<u16> {
    let stem = path.file_stem()?.to_str()?;
    stem.strip_prefix("Map")?.parse().ok()
}

// huge shout out to claude for not making me suffer this one

// ---- RPG::Map structure (only the slivers we need) --------------------------------------------
//
// Unknown ivars (`@data`, `@width`, tilesets, audio, …) are consumed via serde's default
// ignore-unknown-fields behaviour, so we don't have to model the whole map.

#[derive(Deserialize)]
struct RpgMap {
    #[serde(rename = "@events")]
    events: std::collections::HashMap<i64, RpgEvent>,
}

#[derive(Deserialize)]
struct RpgEvent {
    #[serde(rename = "@pages")]
    pages: Vec<RpgPage>,
}

#[derive(Deserialize)]
struct RpgPage {
    #[serde(rename = "@list")]
    list: Vec<RpgCommand>,
}

#[derive(Deserialize)]
struct RpgCommand {
    #[serde(rename = "@parameters")]
    parameters: Vec<Param>,
}

impl RpgMap {
    /// Every string parameter across all event commands. Encounter calls turn up in more than one
    /// command type — bare `Script` commands (codes 355/655) but also `Conditional Branch`
    /// scripts (`if pbWildBattle(...)`, code 111) — so rather than filter by code we scan every
    /// string param. The [`ENCOUNTER_CALL`] regex is specific enough that dialogue won't match.
    fn script_lines(&self) -> impl Iterator<Item = &str> {
        self.events
            .values()
            .flat_map(|e| e.pages.iter())
            .flat_map(|p| p.list.iter())
            .flat_map(|c| c.parameters.iter())
            .filter_map(|p| p.0.as_deref())
    }
}

/// One event-command parameter. We only care about string params (the script text); everything
/// else (ints, arrays, RPG objects) is consumed and discarded.
struct Param(Option<Box<str>>);

impl<'de> Deserialize<'de> for Param {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Vis;
        impl<'de> Visitor<'de> for Vis {
            type Value = Param;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("any event-command parameter")
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E> {
                Ok(Param(Some(v.into())))
            }
            fn visit_string<E>(self, v: String) -> Result<Self::Value, E> {
                Ok(Param(Some(v.into())))
            }
            fn visit_i64<E>(self, _: i64) -> Result<Self::Value, E> {
                Ok(Param(None))
            }
            fn visit_u64<E>(self, _: u64) -> Result<Self::Value, E> {
                Ok(Param(None))
            }
            fn visit_f64<E>(self, _: f64) -> Result<Self::Value, E> {
                Ok(Param(None))
            }
            fn visit_bool<E>(self, _: bool) -> Result<Self::Value, E> {
                Ok(Param(None))
            }
            fn visit_unit<E>(self) -> Result<Self::Value, E> {
                Ok(Param(None))
            }
            fn visit_none<E>(self) -> Result<Self::Value, E> {
                Ok(Param(None))
            }
            fn visit_some<D>(self, d: D) -> Result<Self::Value, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                IgnoredAny::deserialize(d).map(|_| Param(None))
            }
            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                while seq.next_element::<IgnoredAny>()?.is_some() {}
                Ok(Param(None))
            }
            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                Ok(Param(None))
            }
        }

        deserializer.deserialize_any(Vis)
    }
}

#[cfg(test)]
mod test {
    use crate::{
        infinite_fusion::{
            Dex,
            encounters::{EncounterMethod, MapNames},
            map_encounters,
        },
        test::{infinite_fusion_dir, infinite_fusion_hoenn_dir},
    };

    #[test]
    fn scrapes_static_and_gift_encounters_from_maps() {
        let dirs = [infinite_fusion_dir(), infinite_fusion_hoenn_dir()];
        let species = crate::infinite_fusion::species::test::load_species();

        let collected: [Vec<_>; 2] = std::array::from_fn(|i| {
            let map_names = MapNames::from_file(dirs[i].join("Data/MapInfos.rxdata")).unwrap();
            map_encounters::collect(&dirs[i].join("Data"), &species[i], &map_names)
        });

        for (i, rows) in collected.iter().enumerate() {
            assert!(
                !rows.is_empty(),
                "expected to scrape some scripted encounters (game {i})"
            );
        }

        // Mew is a scripted static battle in the base (Kanto) game, never a wild slot.
        let mew = species[0].get_id_of("MEW").unwrap();
        assert!(
            collected[0]
                .iter()
                .any(|e| e.species == mew && e.method == EncounterMethod::Static),
            "expected Mew as a scraped static encounter"
        );
    }
}
