//! Tags species as legendary using Infinite Fusion's own `LEGENDARIES_LIST` (the curated list its randomizer feeds to `is_legendary`).
//! Backs the "exclude legendaries" fusion filter.

use std::{path::Path, sync::LazyLock};

use regex::Regex;
use roaring::RoaringBitmap;

use crate::infinite_fusion::{Dex, DexId, species::SpeciesDex};

const REL_PATH: &str = "025-Randomizer/randomizer.rb";

/// `LEGENDARIES_LIST = [ :A, :B, … ]`. The list holds no nested brackets, so one block grab works.
static LIST: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"LEGENDARIES_LIST\s*=\s*\[([^\]]*)\]").unwrap());

static SYMBOL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r":([A-Z][A-Z0-9_]*)").unwrap());

/// Species indices flagged legendary. Best-effort: a missing script yields an empty set, and
/// symbols absent from this game's species dex (e.g. unmodelled forms) are skipped.
pub fn collect(scripts_dir: &Path, species: &SpeciesDex) -> RoaringBitmap {
    let Ok(text) = std::fs::read_to_string(scripts_dir.join(REL_PATH)) else {
        return RoaringBitmap::new();
    };
    let Some(block) = LIST.captures(&text).map(|c| c.get(1).unwrap().as_str()) else {
        return RoaringBitmap::new();
    };
    SYMBOL
        .captures_iter(block)
        .filter_map(|c| species.get_id_of(&c[1]))
        .map(|id| id.to_u32())
        .collect()
}

#[cfg(test)]
mod test {
    use crate::{
        infinite_fusion::{Dex, DexId, legendaries},
        test::{infinite_fusion_dir, infinite_fusion_hoenn_dir},
    };

    #[test]
    fn flags_known_legendaries() {
        let dirs = [infinite_fusion_dir(), infinite_fusion_hoenn_dir()];
        let species = crate::infinite_fusion::species::test::load_species();

        for i in 0..2 {
            let legendaries = legendaries::collect(&dirs[i].join("Data/Scripts"), &species[i]);

            // Mewtwo and Rayquaza are on the list; Bulbasaur is not.
            let mewtwo = species[i].get_id_of("MEWTWO").unwrap();
            let rayquaza = species[i].get_id_of("RAYQUAZA").unwrap();
            let bulba = species[i].get_id_of("BULBASAUR").unwrap();

            assert!(legendaries.contains(mewtwo.to_u32()), "Mewtwo (game {i})");
            assert!(
                legendaries.contains(rayquaza.to_u32()),
                "Rayquaza (game {i})"
            );
            assert!(
                !legendaries.contains(bulba.to_u32()),
                "Bulbasaur (game {i})"
            );
        }
    }
}
