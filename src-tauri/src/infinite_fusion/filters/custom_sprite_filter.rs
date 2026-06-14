use std::{collections::HashMap, path::Path};

use roaring::RoaringBitmap;

use crate::infinite_fusion::{
    Dex, DexId,
    species::{SpeciesDex, SpeciesId},
};

/// Per-head bitmap of the body indices for which a *base* custom sprite exists, parsed from
/// `Data/sprites/CUSTOM_SPRITES` (see the `sprites` module for the manifest format). Lets the
/// "only fusions with a custom sprite" filter run as a per-head body-set intersection like the
/// other filters. A fusion's custom sprite is keyed by `head.body`, so membership is exact per
/// (head, body) — not separable.
#[derive(Debug, Clone)]
pub struct CustomSpriteIndex {
    by_head: Box<[RoaringBitmap]>,
}

impl CustomSpriteIndex {
    /// Builds from the manifest file. A missing/unreadable manifest yields an empty index (the
    /// filter then matches nothing, which is the honest answer when we can't tell).
    pub fn build(species: &SpeciesDex, manifest_path: &Path) -> Self {
        let mut by_head = vec![RoaringBitmap::new(); species.len()];

        // manifest entries are in in-game dex-number space; map back to our species indices
        let id_to_index: HashMap<u16, u32> = species
            .map()
            .values()
            .enumerate()
            .map(|(i, s)| (s.id_number, i as u32))
            .collect();

        if let Ok(text) = std::fs::read_to_string(manifest_path) {
            for line in text.lines() {
                let stem = line.trim().strip_suffix(".png").unwrap_or(line.trim());
                // base entries only — alt variants like `1.4a` fail the body parse and are skipped
                let Some((head, body)) = stem.split_once('.') else {
                    continue;
                };
                let (Ok(head_id), Ok(body_id)) = (head.parse::<u16>(), body.parse::<u16>()) else {
                    continue;
                };
                if let (Some(&head_idx), Some(&body_idx)) =
                    (id_to_index.get(&head_id), id_to_index.get(&body_id))
                {
                    by_head[head_idx as usize].insert(body_idx);
                }
            }
        }

        Self {
            by_head: by_head.into_boxed_slice(),
        }
    }

    /// Body indices that have a base custom sprite when fused onto `head`.
    pub fn bodies_for_head(&self, head: SpeciesId) -> &RoaringBitmap {
        &self.by_head[head.to_usize()]
    }
}
