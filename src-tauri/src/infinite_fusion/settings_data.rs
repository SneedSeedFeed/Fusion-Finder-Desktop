//! Acquisition sources that live as Ruby constants in `Data/Scripts/001_Settings.rb` rather than
//! in compiled `.dat` data: roaming legendaries (`ROAMING_SPECIES`), Poké Radar-only encounters
//! (`POKE_RADAR_ENCOUNTERS`) and the starter lists (`*_STARTERS`)

use std::{path::Path, sync::LazyLock};

use regex::Regex;

use crate::infinite_fusion::{
    Dex,
    encounters::{Encounter, EncounterMethod, EncounterMode, MapNames},
    species::SpeciesDex,
};

const STARTER_LEVEL: u8 = 5;

/// `[:SPECIES, level, …]` — the first two fields of a `ROAMING_SPECIES` row.
static ROAMING_ROW: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\s*:([A-Z][A-Z0-9_]*)\s*,\s*(\d+)").unwrap());

/// `[map, prob, :SPECIES, min, max?]` — a `POKE_RADAR_ENCOUNTERS` row.
static RADAR_ROW: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[\s*(\d+)\s*,\s*(\d+)\s*,\s*:([A-Z][A-Z0-9_]*)\s*,\s*(\d+)\s*(?:,\s*(\d+))?\s*\]")
        .unwrap()
});

/// A regional starter list assignment, e.g. `KANTO_STARTERS = [:BULBASAUR, …]`. The GRASS/FIRE/
/// WATER lists are deliberately excluded — they just re-list the same species by element.
static STARTER_LIST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:KANTO|JOHTO|HOENN|SINNOH|KALOS)_STARTERS\s*=\s*\[([^\]]*)\]").unwrap()
});

static SPECIES_SYMBOL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r":([A-Z][A-Z0-9_]*)").unwrap());

/// Scrape roaming/radar/starter acquisition rows from `001_Settings.rb` under `scripts_dir`.
/// Best-effort: a missing or unreadable file yields no rows.
pub fn collect(scripts_dir: &Path, species: &SpeciesDex, map_names: &MapNames) -> Vec<Encounter> {
    let Ok(text) = std::fs::read_to_string(scripts_dir.join("001_Settings.rb")) else {
        return Vec::new();
    };

    let mut rows = Vec::new();
    collect_roaming(&text, species, &mut rows);
    collect_radar(&text, species, map_names, &mut rows);
    collect_starters(&text, species, &mut rows);
    rows
}

fn collect_roaming(text: &str, species: &SpeciesDex, rows: &mut Vec<Encounter>) {
    let Some(block) = bracket_block(text, "ROAMING_SPECIES") else {
        return;
    };
    for caps in ROAMING_ROW.captures_iter(block) {
        let Some(id) = species.get_id_of(&caps[1]) else {
            continue; // roaming fusion ids (e.g. "B245H243") aren't species.dat entries
        };
        let level = caps[2].parse().unwrap_or(0);
        rows.push(Encounter {
            species: id,
            route: "Roaming".into(),
            method: EncounterMethod::Roaming,
            chance: 100,
            min_level: level,
            max_level: level,
            mode: EncounterMode::Both,
        });
    }
}

fn collect_radar(
    text: &str,
    species: &SpeciesDex,
    map_names: &MapNames,
    rows: &mut Vec<Encounter>,
) {
    let Some(block) = bracket_block(text, "POKE_RADAR_ENCOUNTERS") else {
        return;
    };
    for caps in RADAR_ROW.captures_iter(block) {
        let Some(id) = species.get_id_of(&caps[3]) else {
            continue;
        };
        let Some(map_id) = caps[1].parse::<u16>().ok() else {
            continue;
        };
        let Some(route) = map_names.get(map_id).cloned() else {
            continue;
        };
        let chance = caps[2].parse().unwrap_or(0);
        let min_level = caps[4].parse().unwrap_or(0);
        // max level is optional; a single-level row repeats the minimum.
        let max_level = caps
            .get(5)
            .and_then(|m| m.as_str().parse().ok())
            .unwrap_or(min_level);
        rows.push(Encounter {
            species: id,
            route,
            method: EncounterMethod::PokeRadar,
            chance,
            min_level,
            max_level,
            mode: EncounterMode::Both,
        });
    }
}

fn collect_starters(text: &str, species: &SpeciesDex, rows: &mut Vec<Encounter>) {
    for list in STARTER_LIST.captures_iter(text) {
        for sym in SPECIES_SYMBOL.captures_iter(&list[1]) {
            let Some(id) = species.get_id_of(&sym[1]) else {
                continue;
            };
            rows.push(Encounter {
                species: id,
                route: "Starter".into(),
                method: EncounterMethod::Gift,
                chance: 100,
                min_level: STARTER_LEVEL,
                max_level: STARTER_LEVEL,
                mode: EncounterMode::Both,
            });
        }
    }
}

/// The `[ … ]` literal assigned to constant `name`, brackets included. Balances nested brackets so
/// it stops at the matching close. Returns `None` if the constant or its array isn't found.
fn bracket_block<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let start = text.find(name)?;
    let open = start + text[start..].find('[')?;
    let mut depth = 0i32;
    for (i, c) in text[open..].char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[open..=open + i]);
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod test {
    use crate::{
        infinite_fusion::{
            Dex,
            encounters::{EncounterMethod, MapNames},
            settings_data,
        },
        test::{infinite_fusion_dir, infinite_fusion_hoenn_dir},
    };

    #[test]
    fn scrapes_settings_constants() {
        let dirs = [infinite_fusion_dir(), infinite_fusion_hoenn_dir()];
        let species = crate::infinite_fusion::species::test::load_species();

        for (dirs, species) in dirs.iter().zip(species.iter()) {
            let map_names = MapNames::from_file(dirs.join("Data/MapInfos.rxdata")).unwrap();
            let rows = settings_data::collect(&dirs.join("Data/Scripts"), species, &map_names);

            // Entei roams in both games.
            let entei = species.get_id_of("ENTEI").unwrap();
            assert!(
                rows.iter()
                    .any(|e| e.species == entei && e.method == EncounterMethod::Roaming),
                "expected Entei as a roaming encounter"
            );

            // The regional starter lists are present in both games' Settings.rb.
            let bulba = species.get_id_of("BULBASAUR").unwrap();
            assert!(
                rows.iter()
                    .any(|e| e.species == bulba && e.method == EncounterMethod::Gift),
                "expected Bulbasaur as a starter"
            );

            // Poké Radar encounters carry a real route + probability.
            assert!(
                rows.iter().any(|e| e.method == EncounterMethod::PokeRadar
                    && e.chance > 0
                    && !e.route.is_empty()),
                "expected a Poké Radar encounter with a route"
            );
        }
    }
}
