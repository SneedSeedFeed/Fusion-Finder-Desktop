//! if you are coming back to this months on and wondering what's going on. it's called a bit-sliced index, google told me theyre good at doing ranges for filters. we have one per stat.
//! claude did a good chunk of the hard maths because my brain can't track maths like it used to.
use roaring::RoaringBitmap;

use crate::infinite_fusion::{
    Dex, DexId, FusionId,
    filters::{StatRanges, and_in},
    species::{SpeciesDex, SpeciesId, base_stats::BaseStats},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FusedStat {
    Hp,
    Atk,
    Def,
    Spa,
    Spd,
    Spe,
    Bst,
}

#[derive(Debug, Clone, Copy)]
pub struct StatRange {
    pub stat: FusedStat,
    pub min: u16,
    pub max: u16,
}

impl StatRange {
    pub fn new(stat: FusedStat, min: u16, max: u16) -> Self {
        Self { stat, min, max }
    }
}

// Each fused stat is `floor(2*dominant/3) + floor(other/3)` (see `base_stats::fuse_calc`), with the
// head dominant for HP/SpA/SpD and the body dominant for Atk/Def/Spe. For a *fixed head* that's
// monotonic in the body, so each per-species column is range-queryable for "the bodies that work".
//
// Because the floors are per-term, BST splits cleanly with no residue:
//   `fused_BST == head_bst_contrib(head) + body_bst_contrib(body)`  (exactly)

/// Head's constant contribution to `fused_BST`: `floor(2*s/3)` over its head-dominant stats plus
/// `floor(s/3)` over its body-dominant stats.
fn head_bst_contrib(s: &BaseStats) -> u16 {
    2 * u16::from(s.hp()) / 3
        + 2 * u16::from(s.spa()) / 3
        + 2 * u16::from(s.spd()) / 3
        + u16::from(s.atk()) / 3
        + u16::from(s.def()) / 3
        + u16::from(s.spe()) / 3
}

/// Body's contribution to `fused_BST`: `floor(s/3)` over the head-dominant stats plus `floor(2*s/3)`
/// over the body-dominant ones. Bit-sliced so it can be range-queried per head.
fn body_bst_contrib(s: &BaseStats) -> u16 {
    u16::from(s.hp()) / 3
        + u16::from(s.spa()) / 3
        + u16::from(s.spd()) / 3
        + 2 * u16::from(s.atk()) / 3
        + 2 * u16::from(s.def()) / 3
        + 2 * u16::from(s.spe()) / 3
}

/// `(column index in base_stats, is the stat head-dominant, head's raw value)` for a single stat.
fn stat_column(stat: FusedStat, head: &BaseStats) -> (usize, bool, i32) {
    match stat {
        FusedStat::Hp => (0, true, i32::from(head.hp())),
        FusedStat::Atk => (1, false, i32::from(head.atk())),
        FusedStat::Def => (2, false, i32::from(head.def())),
        FusedStat::Spa => (3, true, i32::from(head.spa())),
        FusedStat::Spd => (4, true, i32::from(head.spd())),
        FusedStat::Spe => (5, false, i32::from(head.spe())),
        FusedStat::Bst => unreachable!("BST is handled separately"),
    }
}

/// Bit-sliced stat columns keyed by `SpeciesId` (~hundreds of ids, not the `n²` fusions): a stat
/// filter resolves to "the bodies that work for a given head" in a few bitmap ops.
#[derive(Debug, Clone)]
pub struct StatIndex {
    /// hp, atk, def, spa, spd, spe
    base_stats: [StatBsi; 6],
    head_contrib: Box<[u16]>,
    body_contrib: StatBsi,
    base: Box<[BaseStats]>,
}

macro_rules! impl_ranges {
    ([$($stat:ident => $idx:literal),*]) => {
        impl StatIndex {
            $(
                pub fn $stat(&self, min: Option<u8>, max: Option<u8>) -> RoaringBitmap {
                    debug_assert!(min.is_some() || max.is_some());
                    let bsi = &self.base_stats[$idx];
                    let min = min.map(From::from).unwrap_or(bsi.min);
                    let max = max.map(From::from).unwrap_or(bsi.max);
                    debug_assert!(min < max);
                    bsi.range(min, max)
                }
            )*
        }
    };
}

impl_ranges!([hp => 0, atk => 1, def => 2, spa => 3, spd => 4, spe => 5]);

impl StatIndex {
    pub fn build(species: &SpeciesDex) -> Self {
        let base: Vec<_> = species.map().iter().map(|(_, s)| s.base_stats).collect();

        let col =
            |f: fn(&BaseStats) -> u16| StatBsi::build(&base.iter().map(f).collect::<Vec<_>>());
        let base_stats = [
            col(|s| u16::from(s.hp())),
            col(|s| u16::from(s.atk())),
            col(|s| u16::from(s.def())),
            col(|s| u16::from(s.spa())),
            col(|s| u16::from(s.spd())),
            col(|s| u16::from(s.spe())),
        ];

        let head_contrib = base
            .iter()
            .map(head_bst_contrib)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let body_contrib = StatBsi::build(&base.iter().map(body_bst_contrib).collect::<Vec<_>>());

        Self {
            base_stats,
            head_contrib,
            body_contrib,
            base: base.into_boxed_slice(),
        }
    }

    /// The body species that, paired with `head_id`, give a fused `stat` within `[lo, hi]`.
    /// Inverts `fuse_calc` to a body-value interval and range-queries the relevant column
    fn body_set_for_stat(
        &self,
        head_id: usize,
        stat: FusedStat,
        lo: i32,
        hi: i32,
    ) -> RoaringBitmap {
        let (column, body_lo, body_hi) = match stat {
            FusedStat::Bst => {
                // fused_BST == head_contrib[head] + body_contrib[body]
                let hc = i32::from(self.head_contrib[head_id]);
                (&self.body_contrib, lo - hc, hi - hc)
            }
            stat => {
                let head = &self.base[head_id];
                let (idx, head_dominant, hv) = stat_column(stat, head);
                // invert `floor(2*dom/3) + floor(other/3)` for the body value
                let (b_lo, b_hi) = if head_dominant {
                    let c = 2 * hv / 3; // floor(2*head/3), constant for this head
                    (3 * (lo - c), 3 * (hi - c) + 2)
                } else {
                    let c = hv / 3; // floor(head/3)
                    // ceil(x/2) == (x + 1).div_euclid(2), div_euclid floors
                    (
                        (3 * (lo - c) + 1).div_euclid(2),
                        (3 * (hi - c) + 2).div_euclid(2),
                    )
                };
                (&self.base_stats[idx], b_lo, b_hi)
            }
        };

        let body_lo = body_lo.max(0);
        if body_hi < body_lo {
            RoaringBitmap::new() // no body works for this head
        } else {
            column.range(body_lo as u16, body_hi as u16)
        }
    }

    /// The bodies that satisfy every active range in `ranges` for this head; `None` when no stat range is set (no constraint).
    pub fn bodies_for_head(&self, head: SpeciesId, ranges: &StatRanges) -> Option<RoaringBitmap> {
        let head_id = head.to_usize();
        let mut acc: Option<RoaringBitmap> = None;
        for (stat, range) in [
            (
                FusedStat::Hp,
                ranges.hp.map(|r| (i32::from(r.min), i32::from(r.max))),
            ),
            (
                FusedStat::Atk,
                ranges.atk.map(|r| (i32::from(r.min), i32::from(r.max))),
            ),
            (
                FusedStat::Def,
                ranges.def.map(|r| (i32::from(r.min), i32::from(r.max))),
            ),
            (
                FusedStat::Spa,
                ranges.spa.map(|r| (i32::from(r.min), i32::from(r.max))),
            ),
            (
                FusedStat::Spd,
                ranges.spd.map(|r| (i32::from(r.min), i32::from(r.max))),
            ),
            (
                FusedStat::Spe,
                ranges.spe.map(|r| (i32::from(r.min), i32::from(r.max))),
            ),
            (
                FusedStat::Bst,
                ranges.bst.map(|r| (i32::from(r.min), i32::from(r.max))),
            ),
        ] {
            if let Some((lo, hi)) = range {
                and_in(&mut acc, self.body_set_for_stat(head_id, stat, lo, hi));
            }
        }
        acc
    }

    /// Every fusion (`id = head * n + body`) whose fused stats satisfy all of `ranges`
    pub fn filter(&self, ranges: &[StatRange]) -> RoaringBitmap {
        let n = self.base.len();
        let mut result = RoaringBitmap::new();

        for head_id in 0..n {
            let mut bodies: Option<RoaringBitmap> = None;
            for r in ranges {
                let set =
                    self.body_set_for_stat(head_id, r.stat, i32::from(r.min), i32::from(r.max));
                and_in(&mut bodies, set);
            }

            let row = (head_id * n) as u32;
            match bodies {
                Some(bodies) => result.extend(bodies.iter().map(|body| row + body)),
                None => {
                    result.insert_range(row..row + n as u32);
                }
            }
        }

        result
    }

    /// Decode a fusion id (as produced by [`Self::filter`]) back into its head/body species
    pub fn decode(&self, id: u32) -> FusionId {
        let n = self.base.len() as u32;
        FusionId {
            head: SpeciesId::from_u32(id / n),
            body: SpeciesId::from_u32(id % n),
        }
    }
}

/// A bit-sliced index of one `u16` column: `slices[k]` holds every id whose value has bit `k` set
#[derive(Debug, Clone)]
pub struct StatBsi {
    min: u16,
    max: u16,
    // least significant bit first, one per bit of `max`
    slices: Box<[RoaringBitmap]>,
    // every id present
    universe: RoaringBitmap,
}

impl StatBsi {
    fn build(values: &[u16]) -> Self {
        let max = values.iter().copied().max().unwrap_or(0);
        let min = values.iter().copied().min().unwrap_or(0);
        let bits = (u16::BITS - max.leading_zeros()).max(1) as usize;

        let mut slices = vec![RoaringBitmap::new(); bits].into_boxed_slice();
        let mut universe = RoaringBitmap::new();
        for (id, &v) in values.iter().enumerate() {
            let id = id as u32;
            universe.insert(id);
            let mut bitset = v;
            while bitset != 0 {
                slices[bitset.trailing_zeros() as usize].insert(id);
                bitset &= bitset - 1; // clear lowest set bit
            }
        }

        Self {
            min,
            max,
            slices,
            universe,
        }
    }

    /// Ids whose value falls in the inclusive `[lo, hi]` range.
    pub fn range(&self, lo: u16, hi: u16) -> RoaringBitmap {
        if hi < self.min || lo > self.max {
            return RoaringBitmap::new();
        }

        // Out-of-column-bounds sides are the whole univers; only in-bounds bounds need the slices
        // (which also keeps `compare`'s input within the slice width).
        let ge = if lo <= self.min {
            self.universe.clone()
        } else {
            let (_, eq, mut gt) = self.compare(lo);
            gt |= eq;
            gt
        };
        let le = if hi >= self.max {
            self.universe.clone()
        } else {
            let (mut lt, eq, _) = self.compare(hi);
            lt |= eq;
            lt
        };

        ge & le
    }

    /// Bit-sliced comparison against `c`: `(lt, eq, gt)`. Walks most to least significant bit, once an id's bit differs
    /// from `c`'s it's decided as below/above and leaves the still-equal set.
    fn compare(&self, c: u16) -> (RoaringBitmap, RoaringBitmap, RoaringBitmap) {
        let mut lt = RoaringBitmap::new();
        let mut gt = RoaringBitmap::new();
        let mut eq = self.universe.clone();

        for i in (0..self.slices.len() as u32).rev() {
            let slice = &self.slices[i as usize];
            if (c >> i) & 1 == 1 {
                lt |= &eq - slice; // still-equal ids with a 0 here are below c
                eq &= slice;
            } else {
                gt |= &eq & slice; // still-equal ids with a 1 here are above c
                eq -= slice;
            }
        }

        (lt, eq, gt)
    }
}

#[cfg(test)]
mod test {
    use roaring::RoaringBitmap;

    use crate::{
        infinite_fusion::{
            Dex, DexId, GameVersion, InfiniteFusionDex,
            filters::stat_filter::{FusedStat, StatIndex, StatRange, head_bst_contrib},
            species::{SpeciesId, base_stats::BaseStats},
        },
        test::infinite_fusion_dir,
    };

    #[test]
    fn bsi_range_matches_a_manual_scan() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let species = dex.species();
        let index = StatIndex::build(species);
        let n = species.len();

        // raw Attack column (index 1): a bit-sliced range must equal a plain scan of base attack.
        let atk = &index.base_stats[1];
        let (lo, hi) = (60u16, 110u16);

        let bsi: Vec<u32> = atk.range(lo, hi).iter().collect();
        let manual: Vec<u32> = (0..n)
            .filter(|&i| (lo..=hi).contains(&u16::from(index.base[i].atk())))
            .map(|i| i as u32)
            .collect();
        assert_eq!(bsi, manual);

        // unbounded range returns the whole species universe
        assert_eq!(index.body_contrib.range(0, u16::MAX).len() as usize, n);

        // head contribution is a plain per-species scalar (used as a constant in the per-head loop)
        assert_eq!(index.head_contrib.len(), n);
        assert_eq!(index.head_contrib[0], head_bst_contrib(&index.base[0]));
    }

    #[test]
    fn end_to_end_filter_matches_the_materialised_scan() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let species = dex.species();
        let n = species.len();
        let index = StatIndex::build(species);

        // single stats (one head-dominant, one body-dominant) plus BST exercises every branch
        let ranges = [
            StatRange::new(FusedStat::Atk, 90, 140),
            StatRange::new(FusedStat::Hp, 40, 200),
            StatRange::new(FusedStat::Bst, 480, 540),
        ];

        // independent oracle: compute each fusion's stats straight from base_stats via `fuse()`
        let base: Vec<BaseStats> = (0..n)
            .map(|i| species.get_item(SpeciesId::from_usize(i)).base_stats)
            .collect();
        let fused_value = |fused: &BaseStats, stat: FusedStat| -> u16 {
            match stat {
                FusedStat::Hp => fused.hp().into(),
                FusedStat::Atk => fused.atk().into(),
                FusedStat::Def => fused.def().into(),
                FusedStat::Spa => fused.spa().into(),
                FusedStat::Spd => fused.spd().into(),
                FusedStat::Spe => fused.spe().into(),
                FusedStat::Bst => fused.bst(),
            }
        };
        let oracle: RoaringBitmap = (0..n)
            .flat_map(|h| (0..n).map(move |b| (h, b)))
            .filter(|&(h, b)| {
                let fused = base[h].fuse(&base[b]);
                ranges
                    .iter()
                    .all(|r| (r.min..=r.max).contains(&fused_value(&fused, r.stat)))
            })
            .map(|(h, b)| (h * n + b) as u32)
            .collect();

        let result = index.filter(&ranges);
        assert!(!result.is_empty(), "expected some matches");
        assert_eq!(result, oracle); // exact, id for id including BST

        // a result id decodes back to its head/body
        let id = result.iter().next().unwrap();
        let fusion = index.decode(id);
        assert_eq!(
            id,
            (fusion.head.to_usize() * n + fusion.body.to_usize()) as u32
        );
    }
}
