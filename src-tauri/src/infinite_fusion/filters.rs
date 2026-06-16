pub mod ability_filter;
pub mod custom_sprite_filter;
pub mod move_filter;
pub mod stat_filter;
pub mod type_filter;

use roaring::RoaringBitmap;
use serde::{Deserialize, Serialize};
use strum::VariantArray;

use crate::infinite_fusion::{
    Dex, DexId, InfiniteFusionDex,
    abilities::AbilityId,
    filters::type_filter::fused_types,
    moves::MoveId,
    species::{
        SpeciesDetails, SpeciesId,
        base_stats::{BaseStats, Stat, StatDistributions},
    },
    types::TypeDex,
    types::TypeId,
};

/// Intersect `set` into a running per-head body set, where `None` means "all bodies so far".
pub(crate) fn and_in(acc: &mut Option<RoaringBitmap>, set: RoaringBitmap) {
    *acc = Some(match acc.take() {
        Some(existing) => existing & set,
        None => set,
    });
}

/// Fusion-id set for a *separable* filter matches if head **or** body is in `species`
/// (e.g. "has ability A"). Head qualifies -> the whole row; else just the qualifying bodies.
/// Same `head * n + body` scheme as every filter, so combining is a roaring `&`.
pub(crate) fn separable_filter(n: usize, species: &RoaringBitmap) -> RoaringBitmap {
    let n = n as u32;
    let mut result = RoaringBitmap::new();
    for head in 0..n {
        let row = head * n;
        if species.contains(head) {
            result.insert_range(row..row + n);
        } else {
            result.extend(species.iter().map(|body| row + body));
        }
    }
    result
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Filters {
    #[serde(default)]
    pub has_pokemon: Option<HasPokemon>,
    #[serde(default)]
    pub has_type: Box<[TypeId]>, // only uses up to two but [Option<TypeId>; 2] would be annoying
    #[serde(default)]
    pub stat_range: StatRanges,
    #[serde(default)]
    pub has_ability: Option<HasAbility>,
    #[serde(default)]
    pub has_move: Option<HasMove>,
    /// only fusions whose `head.body` has a base custom sprite
    #[serde(default)]
    pub has_custom_sprite: bool,
    #[serde(default)]
    pub mono_type: bool,
    /// defensive type-matchup constraint (weak/resist/immune to the given types)
    #[serde(default)]
    pub defense: Option<DefenseFilter>,
    /// exclude species whose in-game dex number exceeds this (hidden, game-set: Kanto's data
    /// carries Gen-3 species it can't actually fuse, so we cap at the real fusable count)
    #[serde(default)]
    pub block_ids_above: Option<u16>,
    /// drop any fusion with a legendary on either side (per `LEGENDARIES_LIST`)
    #[serde(default)]
    pub exclude_legendaries: bool,
    /// keep only fusions that can still evolve, or only fully-evolved ones (see `EvolutionFilter`)
    #[serde(default)]
    pub evolution: Option<EvolutionFilter>,
    /// drop any fusion with one of these species on either side (a user-curated block list)
    #[serde(default)]
    pub ignored_species: Box<[SpeciesId]>,
}

/// Constrain a fusion by whether it can still evolve. A fusion evolves when *either* component has a
/// forward evolution, so `CanEvolve` keeps a fusion if head **or** body can evolve, while
/// `FullyEvolved` keeps it only when **neither** can.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum EvolutionFilter {
    CanEvolve,
    FullyEvolved,
}

impl Filters {
    /// One per-head pass: intersect every active filter's per-head body set, emit the matches.
    /// Done this way so a broad filter never builds its full fusion-id set it just hands over a per-head species bitmap, so cost tracks the *narrowest* filter, not the largest.
    pub fn apply(&self, dex: &InfiniteFusionDex) -> RoaringBitmap {
        let n = dex.species().len();
        let mut result = RoaringBitmap::new();

        // species (by index) allowed by the id cap, the legendary exclusion and/or the user's block
        // list; both head and body must be allowed. Built once, then intersected per-head like any
        // other constraint.
        let ignored: RoaringBitmap = self.ignored_species.iter().map(|s| s.to_u32()).collect();
        let allowed: Option<RoaringBitmap> = (self.block_ids_above.is_some()
            || self.exclude_legendaries
            || !ignored.is_empty())
        .then(|| {
            let legendaries = dex.legendaries();
            dex.species()
                .map()
                .values()
                .enumerate()
                .filter_map(|(i, s)| {
                    let under_cap = self.block_ids_above.is_none_or(|max| s.id_number <= max);
                    let not_legendary =
                        !self.exclude_legendaries || !legendaries.contains(i as u32);
                    let not_ignored = !ignored.contains(i as u32);
                    (under_cap && not_legendary && not_ignored).then_some(i as u32)
                })
                .collect()
        });

        // species (by index) on the qualifying side of the evolution filter: the still-evolving
        // ones for `CanEvolve`, the fully-evolved ones for `FullyEvolved`. The per-head logic below
        // applies OR vs AND semantics; this set already encodes which membership we're testing for.
        let evo_species: Option<RoaringBitmap> = self.evolution.map(|evo| {
            let can_evolve = |s: &SpeciesDetails| s.evolutions.iter().any(|e| e.target().is_into());
            dex.species()
                .map()
                .values()
                .enumerate()
                .filter_map(|(i, s)| {
                    let keep = match evo {
                        EvolutionFilter::CanEvolve => can_evolve(s),
                        EvolutionFilter::FullyEvolved => !can_evolve(s),
                    };
                    keep.then_some(i as u32)
                })
                .collect()
        });

        for head_id in 0..n {
            let head = SpeciesId::from_usize(head_id);
            let mut bodies: Option<RoaringBitmap> = None;

            if let Some(allowed) = &allowed {
                if !allowed.contains(head_id as u32) {
                    continue; // head species is over the cap -> none of its fusions exist
                }
                and_in(&mut bodies, allowed.clone());
            }

            // pin the chosen species to head, body, or either side
            if let Some(has) = &self.has_pokemon {
                match *has {
                    HasPokemon::Head(p) => {
                        if head != p {
                            continue; // this head isn't the chosen species -> no fusions for you
                        }
                    }
                    HasPokemon::Body(p) => {
                        and_in(&mut bodies, single(p));
                    }
                    HasPokemon::Either(p) => {
                        if head != p {
                            and_in(&mut bodies, single(p));
                        }
                    }
                }
            }
            if let (Some(evo), Some(set)) = (self.evolution, &evo_species) {
                let head_qualifies = set.contains(head_id as u32);
                match evo {
                    // can-evolve: head OR body can evolve -> an evolving head frees all bodies,
                    // otherwise the body must be the one that can evolve.
                    EvolutionFilter::CanEvolve => {
                        if !head_qualifies {
                            and_in(&mut bodies, set.clone());
                        }
                    }
                    // fully-evolved: head AND body must both be fully evolved.
                    EvolutionFilter::FullyEvolved => {
                        if !head_qualifies {
                            continue;
                        }
                        and_in(&mut bodies, set.clone());
                    }
                }
            }
            if let Some(set) = dex.type_index().bodies_for_head(head, &self.has_type) {
                and_in(&mut bodies, set);
            }
            if let Some(set) = dex.stat_index().bodies_for_head(head, &self.stat_range) {
                and_in(&mut bodies, set);
            }
            if let Some(ability) = &self.has_ability
                && let Some(set) = dex.ability_index().bodies_for_head(head, ability)
            {
                and_in(&mut bodies, set);
            }
            if let Some(has_move) = &self.has_move
                && let Some(set) = dex.move_index().bodies_for_head(head, has_move)
            {
                and_in(&mut bodies, set);
            }
            if self.has_custom_sprite {
                and_in(
                    &mut bodies,
                    dex.custom_sprite_index().bodies_for_head(head).clone(),
                );
            }
            if let Some(set) = defense_bodies(dex, head, self.mono_type, self.defense.as_ref()) {
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
}

/// A single-element bitmap, for pinning one species.
fn single(id: SpeciesId) -> RoaringBitmap {
    let mut set = RoaringBitmap::new();
    set.insert(id.to_u32());
    set
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub enum HasPokemon {
    Either(SpeciesId),
    Head(SpeciesId),
    Body(SpeciesId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum DefenseRelation {
    Weak,
    Resist,
    Immune,
}

/// Constrain a fusion's defensive matchups: it must hold `relation` against every given type
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DefenseFilter {
    pub relation: DefenseRelation,
    pub types: Box<[TypeId]>,
}

/// Bodies (by index) whose fusion with `head` satisfies the mono-type / defensive constraints returning `None` when neither is active
fn defense_bodies(
    dex: &InfiniteFusionDex,
    head: SpeciesId,
    mono: bool,
    defense: Option<&DefenseFilter>,
) -> Option<RoaringBitmap> {
    if !mono && defense.is_none() {
        return None;
    }
    let types = dex.types();
    let head_sp = dex.species().get_item(head);
    let mut set = RoaringBitmap::new();
    for body_idx in 0..dex.species().len() {
        let body_sp = dex.species().get_item(SpeciesId::from_usize(body_idx));
        let (t1, t2) = fused_types(head_sp, body_sp, types);
        if mono && t2.is_some() {
            continue;
        }
        if let Some(def) = defense
            && !def
                .types
                .iter()
                .all(|&atk| matches_relation(types, t1, t2, atk, def.relation))
        {
            continue;
        }
        set.insert(body_idx as u32);
    }
    Some(set)
}

/// `attack`'s effectiveness against defender type `d`, in quarter units (0 = immune, 2 = 0.5×, 4 = 1×, 8 = 2×).
fn type_factor(types: &TypeDex, d: TypeId, attack: TypeId) -> u32 {
    let det = types.get_item(d);
    if det.immunities.contains(attack) {
        0
    } else if det.weaknesses.contains(attack) {
        8
    } else if det.resistances.contains(attack) {
        2
    } else {
        4
    }
}

fn matches_relation(
    types: &TypeDex,
    t1: TypeId,
    t2: Option<TypeId>,
    attack: TypeId,
    relation: DefenseRelation,
) -> bool {
    let mut quarters = type_factor(types, t1, attack);
    if let Some(t2) = t2 {
        quarters = quarters * type_factor(types, t2, attack) / 4;
    }
    match relation {
        DefenseRelation::Weak => quarters > 4,
        DefenseRelation::Resist => quarters > 0 && quarters < 4,
        DefenseRelation::Immune => quarters == 0,
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum HasAbility {
    Normal(AbilityId),
    Hidden(AbilityId),
    Either(AbilityId),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HasMove {
    pub egg: bool,
    pub level: bool,
    pub tutor: bool,
    pub moves: Box<[MoveId]>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, Default)]
pub struct StatRanges {
    #[serde(default)]
    pub hp: Option<StatRange<u8>>,
    #[serde(default)]
    pub atk: Option<StatRange<u8>>,
    #[serde(default)]
    pub def: Option<StatRange<u8>>,
    #[serde(default)]
    pub spa: Option<StatRange<u8>>,
    #[serde(default)]
    pub spd: Option<StatRange<u8>>,
    #[serde(default)]
    pub spe: Option<StatRange<u8>>,
    #[serde(default)]
    pub bst: Option<StatRange<u16>>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct StatRange<T> {
    pub min: T,
    pub max: T,
}

/// A single sortable quantity derived from a fusion
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
pub enum Metric {
    Hp,
    Atk,
    Def,
    Spa,
    Spd,
    Spe,
    Bst,
    // synergy: how much better is this fusion than the parents it's made from?
    // additive surplus over both parents: (fused − head) + (fused − body)
    // large negatives should offset large positives, so Caterpie + Necrozma doesn't top the list
    SumOfParts,
    // fused BST relative to the average parent BST (`1.0` = neutral)
    // scale-free, leans toward hidden gems since a weak pair gaining 10% ranks with a strong pair gaining 10%
    SynergyRatio,
    // fused BST minus the stronger parent, positive only when the fusion outright beats both
    SurplusOverBest,
    // SumOfParts but on percentile-normalized stats (each stat -> its rank in the field), hopefully reducing the influence of the extremes
    BalancedSynergy,
    // effective health metrics
    PhysicalEHp,
    SpecialEHp,
    // harmonic mean of Physical and Special effective health to reward generally bulky pokemon
    CombinedEHp,
    // type adjusted effective health, scaled by the typing's defensive multipliers
    TAPhysicalEHp,
    TASpecialEHp,
    TACombinedEHp,
    // sweep metrics: an attacking stat scaled by how usable its speed is
    PhysicalSweep,
    SpecialSweep,
    // the better attacking side scaled by speed
    CombinedSweep,
    // the harmonic mean of both attacking sides scaled by speed (rewards mixed attackers like salamence)
    MixedSweep,
    // sweep metrics adjusted by type coverage
    TAPhysicalSweep,
    TASpecialSweep,
    TACombinedSweep,
    TAMixedSweep,
}

/// Harmonic mean of two values. Pulled toward the smaller of the two, so a lopsided pair scores far below a balanced one
fn harmonic_mean(a: f32, b: f32) -> f32 {
    let sum = a + b;
    if sum == 0.0 {
        return 0.0;
    }
    2.0 * a * b / sum
}

/// Which base stats count toward the synergy metrics. Lets a physical attacker ignore synergy gained
/// from Sp. Atk, a Trick Room pick ignore Speed, etc. An empty selection is treated as "all".
#[derive(Debug, Clone, Copy)]
pub struct StatMask(u8); // bit `stat as u8` set = included

impl StatMask {
    pub const ALL: StatMask = StatMask(0b0011_1111);

    pub fn from_stats(stats: &[Stat]) -> Self {
        if stats.is_empty() {
            return Self::ALL;
        }
        StatMask(stats.iter().fold(0, |bits, &s| bits | 1 << s as u8))
    }

    fn contains(self, stat: Stat) -> bool {
        self.0 & (1 << stat as u8) != 0
    }
}

/// Logistic mapping from speed to its estimated offensive usefulness.
/// Mapping is calibrated to the dex's own speed distribution.
/// Centred on the 50th percentile and tapers at p10 and p90, so a handful of speed freaks can't stretch it.
/// Is intended to reward attackers at "efficient" speed stats like Garchomp who have enough speed to outspeed and kill but don't waste it like Regieleki
#[derive(Debug, Clone, Copy)]
pub struct SpeedCurve {
    pub(crate) midpoint: f32, // median speed -> factor 0.5
    pub(crate) steepness: f32,
}

impl SpeedCurve {
    pub(crate) fn from_speed(dist: &StatDistributions) -> Self {
        let p = |q| f32::from(dist.percentile(Stat::Spe, q));
        let (p10, p50, p90) = (p(0.10), p(0.50), p(0.90));
        // endpoint usefulness: a p10 mon ~= EDGE, a p90 mon ~= 1 - EDGE
        const EDGE: f32 = 0.02;
        let spread = (p90 - p10).max(1.0); // guard a degenerate (single-value) distribution
        let steepness = 2.0 * ((1.0 - EDGE) / EDGE).ln() / spread;
        Self {
            midpoint: p50,
            steepness,
        }
    }

    pub(crate) fn factor(&self, spe: f32) -> f32 {
        1.0 / (1.0 + (-self.steepness * (spe - self.midpoint)).exp())
    }
}

/// Precomputed defensive multiplier for every typing
struct TypeDefense {
    n_types: usize,
    mono: Box<[f32]>, // indexed by t1
    dual: Box<[f32]>, // indexed by t1 * n_types + t2 (symmetric, stored full for direct lookup)
}

impl TypeDefense {
    fn build(types: &TypeDex) -> Self {
        let n = types.len();
        let attackers: Box<[TypeId]> = (0..n).map(TypeId::from_usize).collect();
        let multiplier = |t1: TypeId, t2: Option<TypeId>| {
            let total_quarters: u32 = attackers
                .iter()
                .map(|&atk| {
                    let q = type_factor(types, t1, atk);
                    match t2 {
                        Some(t2) => q * type_factor(types, t2, atk) / 4,
                        None => q,
                    }
                })
                .sum();
            if total_quarters == 0 {
                4.0 // immune to everything, cap rather than divide by 0
            } else {
                4.0 * n as f32 / total_quarters as f32
            }
        };

        let mono = attackers.iter().map(|&t1| multiplier(t1, None)).collect();
        let mut dual = vec![0.0f32; n * n];
        for t1 in 0..n {
            for t2 in 0..n {
                dual[t1 * n + t2] =
                    multiplier(TypeId::from_usize(t1), Some(TypeId::from_usize(t2)));
            }
        }
        Self {
            n_types: n,
            mono,
            dual: dual.into_boxed_slice(),
        }
    }

    fn factor(&self, t1: TypeId, t2: Option<TypeId>) -> f32 {
        match t2 {
            Some(t2) => self.dual[t1.to_usize() * self.n_types + t2.to_usize()],
            None => self.mono[t1.to_usize()],
        }
    }
}

/// Precomputed offensive multiplier for every typing: how well its STAB coverage fares against the field
struct TypeOffense {
    n_types: usize,
    mono: Box<[f32]>, // indexed by t1
    dual: Box<[f32]>, // indexed by t1 * n_types + t2 (symmetric, stored full for direct lookup)
}

impl TypeOffense {
    fn build(types: &TypeDex) -> Self {
        let n = types.len();
        let defenders: Box<[TypeId]> = (0..n).map(TypeId::from_usize).collect();
        let multiplier = |t1: TypeId, t2: Option<TypeId>| {
            let total_quarters: u32 = defenders
                .iter()
                .map(|&d| {
                    let q1 = type_factor(types, d, t1);
                    match t2 {
                        // best STAB against this defender
                        Some(t2) => q1.max(type_factor(types, d, t2)),
                        None => q1,
                    }
                })
                .sum();
            // 4 quarters per defender is neutral, so normalise so all-neutral coverage scores 1.0
            total_quarters as f32 / (4 * n) as f32
        };

        let mono = defenders.iter().map(|&t1| multiplier(t1, None)).collect();
        let mut dual = vec![0.0f32; n * n];
        for t1 in 0..n {
            for t2 in 0..n {
                dual[t1 * n + t2] =
                    multiplier(TypeId::from_usize(t1), Some(TypeId::from_usize(t2)));
            }
        }
        Self {
            n_types: n,
            mono,
            dual: dual.into_boxed_slice(),
        }
    }

    fn factor(&self, t1: TypeId, t2: Option<TypeId>) -> f32 {
        match t2 {
            Some(t2) => self.dual[t1.to_usize() * self.n_types + t2.to_usize()],
            None => self.mono[t1.to_usize()],
        }
    }
}

/// Per-search evaluation context for metrics: the dex plus any distribution-derived calibration,
/// built once so the hot per-fusion loop stays O(1). Future metrics needing other distribution stats
/// add fields here rather than recomputing per fusion.
struct MetricContext<'a> {
    dex: &'a InfiniteFusionDex,
    type_defense: TypeDefense,
    type_offense: TypeOffense,
    /// the synergy stats, expanded once so the per-fusion loop iterates only the included ones
    synergy_stats: Box<[Stat]>,
    raw_size: Box<[f32]>,
    norm_size: Box<[f32]>,
}

impl<'a> MetricContext<'a> {
    fn new(dex: &'a InfiniteFusionDex, synergy_mask: StatMask) -> Self {
        let dist = dex.species().stat_distributions();
        let rank = dist.rank_table();
        let synergy_stats: Box<[Stat]> = Stat::VARIANTS
            .iter()
            .copied()
            .filter(|&s| synergy_mask.contains(s))
            .collect();

        let (raw_size, norm_size) = dex
            .base_stats
            .iter()
            .map(|s| {
                (
                    synergy_stats
                        .iter()
                        .map(|&st| f32::from(s.get(st)))
                        .sum::<f32>(),
                    synergy_stats
                        .iter()
                        .map(|&st| rank[st as usize][usize::from(s.get(st))])
                        .sum::<f32>(),
                )
            })
            .collect::<(Vec<f32>, Vec<f32>)>();

        let raw_size = raw_size.into_boxed_slice();
        let norm_size = norm_size.into_boxed_slice();
        Self {
            dex,
            type_defense: TypeDefense::build(dex.types()),
            type_offense: TypeOffense::build(dex.types()),
            synergy_stats,
            raw_size,
            norm_size,
        }
    }
}

impl Metric {
    fn fused_value(self, ctx: &MetricContext, head: u32, body: u32) -> f32 {
        let (h, b) = (head as usize, body as usize);
        let fused = ctx.dex.base_stats[h].fuse(&ctx.dex.base_stats[b]);
        let hp = f32::from(fused.hp());
        let (atk, spa) = (f32::from(fused.atk()), f32::from(fused.spa()));
        let (phys_ehp, spec_ehp) = (hp * f32::from(fused.def()), hp * f32::from(fused.spd()));
        // lazily evaluated since only the relevant arms need them
        let sweep_speed = || ctx.dex.speed_curve().factor(f32::from(fused.spe()));
        // only the type-adjusted metrics need the full `SpeciesDetails` (for its typing), so the
        // pointer-chase into it stays behind this closure rather than on every fusion's hot path
        let type_defense = || {
            let dex = ctx.dex;
            let head_sp = dex.species().get_item(SpeciesId::from_u32(head));
            let body_sp = dex.species().get_item(SpeciesId::from_u32(body));
            let (t1, t2) = fused_types(head_sp, body_sp, dex.types());
            ctx.type_defense.factor(t1, t2)
        };
        let type_offense = || {
            let dex = ctx.dex;
            let head_sp = dex.species().get_item(SpeciesId::from_u32(head));
            let body_sp = dex.species().get_item(SpeciesId::from_u32(body));
            let (t1, t2) = fused_types(head_sp, body_sp, dex.types());
            ctx.type_offense.factor(t1, t2)
        };
        // synergy "size" of the fused stat block, summed over the selected stats only. `raw` is the
        // plain stat total (full mask == BST); `normalized` maps each stat onto its precomputed rank
        // in the field so extreme raw stats compress (`BalancedSynergy`). The two parents' sizes are
        // pre-summed per species in the context (`raw_size` / `norm_size`).
        let raw = |s: &BaseStats| -> f32 {
            ctx.synergy_stats
                .iter()
                .map(|&st| f32::from(s.get(st)))
                .sum()
        };
        let normalized = |s: &BaseStats| -> f32 {
            ctx.synergy_stats
                .iter()
                .map(|&st| ctx.dex.rank[st as usize][usize::from(s.get(st))])
                .sum()
        };
        match self {
            Metric::Hp => hp,
            Metric::Atk => atk,
            Metric::Def => fused.def().into(),
            Metric::Spa => spa,
            Metric::Spd => fused.spd().into(),
            Metric::Spe => fused.spe().into(),
            Metric::Bst => fused.bst().into(),
            Metric::SumOfParts => {
                let f = raw(&fused);
                (f - ctx.raw_size[h]) + (f - ctx.raw_size[b])
            }
            Metric::SynergyRatio => {
                let avg_parent = (ctx.raw_size[h] + ctx.raw_size[b]) / 2.0;
                if avg_parent == 0.0 {
                    0.0
                } else {
                    raw(&fused) / avg_parent
                }
            }
            Metric::SurplusOverBest => raw(&fused) - ctx.raw_size[h].max(ctx.raw_size[b]),
            Metric::BalancedSynergy => {
                let f = normalized(&fused);
                (f - ctx.norm_size[h]) + (f - ctx.norm_size[b])
            }
            Metric::PhysicalEHp => phys_ehp,
            Metric::SpecialEHp => spec_ehp,
            Metric::CombinedEHp => harmonic_mean(phys_ehp, spec_ehp),
            Metric::TAPhysicalEHp => phys_ehp * type_defense(),
            Metric::TASpecialEHp => spec_ehp * type_defense(),
            Metric::TACombinedEHp => harmonic_mean(phys_ehp, spec_ehp) * type_defense(),
            Metric::PhysicalSweep => atk * sweep_speed(),
            Metric::SpecialSweep => spa * sweep_speed(),
            Metric::CombinedSweep => atk.max(spa) * sweep_speed(),
            Metric::MixedSweep => harmonic_mean(atk, spa) * sweep_speed(),
            Metric::TAPhysicalSweep => atk * sweep_speed() * type_offense(),
            Metric::TASpecialSweep => spa * sweep_speed() * type_offense(),
            Metric::TACombinedSweep => atk.max(spa) * sweep_speed() * type_offense(),
            Metric::TAMixedSweep => harmonic_mean(atk, spa) * sweep_speed() * type_offense(),
        }
    }
}

/// Order the matching fusion ids ascending. Direction is handled on the front to avoid re-computing searches.
///
/// * `metric == None` -> dex order (technically species load order)
/// * `metric == Some(m)`, `secondary == None` -> by the fused value of `m`.
/// * `metric == Some(m)`, `secondary == Some(d)` -> by the ratio `m / d`
pub fn order_matches(
    dex: &InfiniteFusionDex,
    matches: RoaringBitmap,
    metric: Option<Metric>,
    secondary: Option<Metric>,
    synergy_stats: StatMask,
) -> Box<[u32]> {
    let Some(primary) = metric else {
        // RoaringBitmap iterates ascending, which is exactly dex order
        return matches.iter().collect();
    };
    let n = dex.species().len() as u32;

    let ctx = MetricContext::new(dex, synergy_stats); // distribution-derived calibration, once

    // one f32 key per fusion: the metric itself, or the metric/secondary ratio
    let key_of = |id: u32| -> (f32, u32) {
        let value = primary.fused_value(&ctx, id / n, id % n);
        let key = match secondary {
            Some(denom) => value / denom.fused_value(&ctx, id / n, id % n),
            None => value,
        };
        (key, id)
    };

    // most of the below is the expensive part of the workload and is typically faster under rayon
    // its feature flagged so if users dont like it they switch to the non-mt version no problem
    #[cfg(feature = "mt")]
    let mut keyed: Vec<(f32, u32)> = {
        use rayon::prelude::*;
        // collect the bitmap to an indexable slice first; the iterator itself isn't parallelisable
        matches
            .iter()
            .collect::<Vec<u32>>()
            .into_par_iter()
            .map(key_of)
            .collect()
    };
    #[cfg(not(feature = "mt"))]
    let mut keyed: Vec<(f32, u32)> = matches.iter().map(key_of).collect();

    // according to informal benchmarks this sort takes the majority of processing time
    #[cfg(feature = "mt")]
    {
        use rayon::prelude::*;
        keyed.par_sort_unstable_by(|a, b| a.0.total_cmp(&b.0));
    }
    #[cfg(not(feature = "mt"))]
    keyed.sort_unstable_by(|a, b| a.0.total_cmp(&b.0));

    keyed.into_iter().map(|(_, id)| id).collect()
}

#[cfg(test)]
mod test {
    use roaring::RoaringBitmap;
    use strum::VariantArray;

    use super::{
        DefenseFilter, DefenseRelation, EvolutionFilter, Filters, HasAbility, HasMove, HasPokemon,
        Metric, StatMask, StatRange, StatRanges, ability_filter::AbilitySource,
        move_filter::MoveSource, order_matches, separable_filter, stat_filter::FusedStat,
        stat_filter::StatRange as TaggedRange,
    };
    use crate::{
        infinite_fusion::{
            Dex, DexId, GameVersion, InfiniteFusionDex,
            species::{SpeciesId, base_stats::Stat},
        },
        test::infinite_fusion_dir,
    };

    /// The per-head `apply` must equal the naive `&` of the (separately brute-force tested) standalone filters.
    #[test]
    fn apply_matches_naive_combination() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();

        let grass = dex.types().get_id_of("GRASS").unwrap();
        let overgrow = dex.abilities().get_id_of("OVERGROW").unwrap();
        let tackle = dex.moves().get_id_of("TACKLE").unwrap();

        let filters = Filters {
            has_pokemon: None,
            has_type: [grass].into(),
            stat_range: StatRanges {
                atk: Some(StatRange { min: 40, max: 160 }),
                bst: Some(StatRange { min: 300, max: 600 }),
                ..Default::default()
            },
            has_ability: Some(HasAbility::Either(overgrow)),
            has_move: Some(HasMove {
                egg: false,
                level: true,
                tutor: false,
                moves: [tackle].into(),
            }),
            ..Default::default()
        };

        let naive = dex.stat_index().filter(&[
            TaggedRange::new(FusedStat::Atk, 40, 160),
            TaggedRange::new(FusedStat::Bst, 300, 600),
        ]) & dex.type_index().filter(grass)
            & dex.ability_index().filter(overgrow, AbilitySource::Any)
            & dex.move_index().filter(tackle, MoveSource::LevelUp);

        let optimised = filters.apply(&dex);
        assert!(!optimised.is_empty());
        assert_eq!(optimised, naive);
    }

    #[test]
    fn apply_has_pokemon_matches_a_separable_filter() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let n = dex.species().len();
        let bulbasaur = dex.species().get_id_of("BULBASAUR").unwrap();

        let filters = Filters {
            has_pokemon: Some(HasPokemon::Either(bulbasaur)),
            ..Default::default()
        };

        let mut only = RoaringBitmap::new();
        only.insert(bulbasaur.to_u32());
        let naive = separable_filter(n, &only);

        let optimised = filters.apply(&dex);
        assert!(!optimised.is_empty());
        assert_eq!(optimised, naive);
    }

    #[test]
    fn has_pokemon_head_body_either() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let n = dex.species().len() as u32;
        let bulba = dex.species().get_id_of("BULBASAUR").unwrap().to_u32();

        let head = Filters {
            has_pokemon: Some(HasPokemon::Head(
                dex.species().get_id_of("BULBASAUR").unwrap(),
            )),
            ..Default::default()
        }
        .apply(&dex);
        let body = Filters {
            has_pokemon: Some(HasPokemon::Body(
                dex.species().get_id_of("BULBASAUR").unwrap(),
            )),
            ..Default::default()
        }
        .apply(&dex);
        let either = Filters {
            has_pokemon: Some(HasPokemon::Either(
                dex.species().get_id_of("BULBASAUR").unwrap(),
            )),
            ..Default::default()
        }
        .apply(&dex);

        assert!(head.iter().all(|id| id / n == bulba));
        assert!(body.iter().all(|id| id % n == bulba));
        assert!(either.iter().all(|id| id / n == bulba || id % n == bulba));
        // either is the union of head and body, overlapping only at bulbasaur/bulbasaur
        assert_eq!(either.len(), head.len() + body.len() - 1);
    }

    #[test]
    fn defense_immune_and_mono() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let n = dex.species().len() as u32;
        let electric = dex.types().get_id_of("ELECTRIC").unwrap();
        let diglett = dex.species().get_id_of("DIGLETT").unwrap().to_u32();
        let bulba = dex.species().get_id_of("BULBASAUR").unwrap().to_u32();
        let self_fusion = |s: u32| s * n + s;

        // Diglett/Diglett is pure Ground thus immune to Electric and mono-type
        let immune = Filters {
            defense: Some(DefenseFilter {
                relation: DefenseRelation::Immune,
                types: [electric].into(),
            }),
            ..Default::default()
        }
        .apply(&dex);
        assert!(immune.contains(self_fusion(diglett)));

        let mono = Filters {
            mono_type: true,
            ..Default::default()
        }
        .apply(&dex);
        assert!(mono.contains(self_fusion(diglett)));
        // Bulbasaur/Bulbasaur is Grass/Poison (dual) → excluded
        assert!(!mono.contains(self_fusion(bulba)));
    }

    #[test]
    fn block_ids_above_caps_both_components() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let allowed = dex
            .species()
            .map()
            .values()
            .filter(|s| s.id_number <= 501)
            .count();
        // Kanto carries Gen-3 species (id > 501) it can't fuse
        assert!(
            allowed < dex.species().len(),
            "expected some capped species"
        );

        let result = Filters {
            block_ids_above: Some(501),
            ..Default::default()
        }
        .apply(&dex);

        // exactly the allowed×allowed grid — neither head nor body may exceed the cap
        assert_eq!(result.len() as usize, allowed * allowed);
    }

    #[test]
    fn exclude_legendaries_drops_either_side() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let n = dex.species().len() as u32;
        let mewtwo = dex.species().get_id_of("MEWTWO").unwrap();
        let bulba = dex.species().get_id_of("BULBASAUR").unwrap();

        let result = Filters {
            exclude_legendaries: true,
            ..Default::default()
        }
        .apply(&dex);

        // Bulbasaur/Bulbasaur survives; anything pairing the legendary Mewtwo (as head or body) is
        // dropped.
        assert!(result.contains(bulba.to_u32() * n + bulba.to_u32()));
        assert!(!result.contains(mewtwo.to_u32() * n + bulba.to_u32()));
        assert!(!result.contains(bulba.to_u32() * n + mewtwo.to_u32()));
        assert!(!result.contains(mewtwo.to_u32() * n + mewtwo.to_u32()));
    }

    #[test]
    fn has_custom_sprite_is_a_strict_non_empty_subset() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let n = dex.species().len() as u32;

        let all = Filters::default().apply(&dex);
        let custom = Filters {
            has_custom_sprite: true,
            ..Default::default()
        }
        .apply(&dex);

        assert!(!custom.is_empty());
        assert!(custom.len() < all.len());
        assert!(custom.is_subset(&all));

        // bulbasaur(head) + charmander(body) is a hand-drawn sprite (manifest has `1.4.png`)
        let bulbasaur = dex.species().get_id_of("BULBASAUR").unwrap();
        let charmander = dex.species().get_id_of("CHARMANDER").unwrap();
        assert!(custom.contains(bulbasaur.to_u32() * n + charmander.to_u32()));
    }

    #[test]
    fn sort_by_stat_orders_correctly_and_preserves_the_set() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let n = dex.species().len() as u32;
        let bulbasaur = dex.species().get_id_of("BULBASAUR").unwrap();

        let matches = Filters {
            has_pokemon: Some(HasPokemon::Either(bulbasaur)),
            ..Default::default()
        }
        .apply(&dex);

        let ordered = order_matches(
            &dex,
            matches.clone(),
            Some(Metric::Bst),
            None,
            StatMask::ALL,
        );
        // same set, just reordered
        assert_eq!(ordered.len(), matches.len() as usize);
        assert_eq!(ordered.iter().copied().collect::<RoaringBitmap>(), matches);

        let bst = |id: u32| {
            let head = dex
                .species()
                .get_item(SpeciesId::from_u32(id / n))
                .base_stats;
            let body = dex
                .species()
                .get_item(SpeciesId::from_u32(id % n))
                .base_stats;
            head.fuse(&body).bst()
        };
        // ascending: non-decreasing fused BST (the frontend reverses this for descending)
        assert!(ordered.windows(2).all(|w| bst(w[0]) <= bst(w[1])));
        // no metric (dex order): strictly ascending id order
        assert!(
            order_matches(&dex, matches.clone(), None, None, StatMask::ALL)
                .windows(2)
                .all(|w| w[0] < w[1])
        );
    }

    #[test]
    fn evolution_filter_partitions_into_can_and_cannot_evolve() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let n = dex.species().len() as u32;
        let can_evolve = |idx: u32| {
            dex.species()
                .get_item(SpeciesId::from_u32(idx))
                .evolutions
                .iter()
                .any(|e| e.target().is_into())
        };

        let can = Filters {
            evolution: Some(EvolutionFilter::CanEvolve),
            ..Default::default()
        }
        .apply(&dex);
        // every kept fusion has at least one evolving component
        assert!(
            can.iter()
                .all(|id| can_evolve(id / n) || can_evolve(id % n))
        );

        let cannot = Filters {
            evolution: Some(EvolutionFilter::FullyEvolved),
            ..Default::default()
        }
        .apply(&dex);
        // every kept fusion has no evolving component
        assert!(
            cannot
                .iter()
                .all(|id| !can_evolve(id / n) && !can_evolve(id % n))
        );

        // the two are a disjoint, exhaustive partition of the unfiltered set
        let all = Filters::default().apply(&dex);
        assert!((can.clone() & cannot.clone()).is_empty());
        assert_eq!(can.len() + cannot.len(), all.len());
    }

    #[test]
    fn sort_by_ratio_orders_by_metric_quotient() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let n = dex.species().len() as u32;
        let bulbasaur = dex.species().get_id_of("BULBASAUR").unwrap();

        let matches = Filters {
            has_pokemon: Some(HasPokemon::Either(bulbasaur)),
            ..Default::default()
        }
        .apply(&dex);

        // atk-to-def ratio of a fusion, as an exact rational compared via cross-multiplication
        let ratio = |id: u32| {
            let head = dex
                .species()
                .get_item(SpeciesId::from_u32(id / n))
                .base_stats;
            let body = dex
                .species()
                .get_item(SpeciesId::from_u32(id % n))
                .base_stats;
            let fused = head.fuse(&body);
            (u64::from(fused.atk()), u64::from(fused.def()))
        };

        let asc = order_matches(
            &dex,
            matches.clone(),
            Some(Metric::Atk),
            Some(Metric::Def),
            StatMask::ALL,
        );
        // same set, just reordered
        assert_eq!(asc.iter().copied().collect::<RoaringBitmap>(), matches);
        // ascending: non-decreasing atk/def ratio (a/b <= c/d  <=>  a*d <= c*b), checked against an
        // exact integer cross-multiplication oracle to confirm the f32 keys don't flip the order
        assert!(asc.windows(2).all(|w| {
            let (an, ad) = ratio(w[0]);
            let (bn, bd) = ratio(w[1]);
            an * bd <= bn * ad
        }));
    }

    #[test]
    fn combined_ehp_sorts_by_harmonic_bulk_and_punishes_lopsidedness() {
        // harmonic mean of equal sides is the side; of a lopsided pair it sits near the smaller.
        assert_eq!(super::harmonic_mean(100.0, 100.0), 100.0);
        assert_eq!(super::harmonic_mean(0.0, 0.0), 0.0);
        assert!(super::harmonic_mean(10.0, 1000.0) < super::harmonic_mean(100.0, 100.0)); // ~19.8 < 100
        assert_eq!(super::harmonic_mean(0.0, 1000.0), 0.0); // a paper side tanks the whole score

        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let n = dex.species().len() as u32;
        let bulbasaur = dex.species().get_id_of("BULBASAUR").unwrap();

        let matches = Filters {
            has_pokemon: Some(HasPokemon::Either(bulbasaur)),
            ..Default::default()
        }
        .apply(&dex);

        let combined_ehp = |id: u32| {
            let head = dex
                .species()
                .get_item(SpeciesId::from_u32(id / n))
                .base_stats;
            let body = dex
                .species()
                .get_item(SpeciesId::from_u32(id % n))
                .base_stats;
            let fused = head.fuse(&body);
            let hp = f32::from(fused.hp());
            super::harmonic_mean(hp * f32::from(fused.def()), hp * f32::from(fused.spd()))
        };

        let asc = order_matches(
            &dex,
            matches.clone(),
            Some(Metric::CombinedEHp),
            None,
            StatMask::ALL,
        );
        // same set, just reordered, ordered by non-decreasing combined eHP
        assert_eq!(asc.iter().copied().collect::<RoaringBitmap>(), matches);
        assert!(
            asc.windows(2)
                .all(|w| combined_ehp(w[0]) <= combined_ehp(w[1]))
        );
    }

    #[test]
    fn combined_sweep_sorts_by_speed_scaled_offence() {
        // a logistic speed curve (midpoint 90) rises monotonically, hits 0.5 at the midpoint, and saturates toward 0/1 at the extremes
        let curve = super::SpeedCurve {
            midpoint: 90.0,
            steepness: 0.1,
        };
        assert!((curve.factor(90.0) - 0.5).abs() < 1e-6);
        assert!(
            curve.factor(40.0) < curve.factor(90.0) && curve.factor(90.0) < curve.factor(140.0)
        );
        assert!(curve.factor(30.0) < 0.05 && curve.factor(150.0) > 0.95);
        // with diminishing returns: a step nearer the midpoint gains more than one further out.
        assert!(
            (curve.factor(120.0) - curve.factor(110.0))
                < (curve.factor(100.0) - curve.factor(90.0))
        );

        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let n = dex.species().len() as u32;
        let bulbasaur = dex.species().get_id_of("BULBASAUR").unwrap();
        // the real, data-calibrated curve the metric uses
        let curve = super::SpeedCurve::from_speed(dex.species().stat_distributions());

        let matches = Filters {
            has_pokemon: Some(HasPokemon::Either(bulbasaur)),
            ..Default::default()
        }
        .apply(&dex);

        // (atk, spa, speed_factor) of a fusion, recomputed independently of the metric code
        let parts = |id: u32| {
            let head = dex
                .species()
                .get_item(SpeciesId::from_u32(id / n))
                .base_stats;
            let body = dex
                .species()
                .get_item(SpeciesId::from_u32(id % n))
                .base_stats;
            let fused = head.fuse(&body);
            let sf = curve.factor(f32::from(fused.spe()));
            (f32::from(fused.atk()), f32::from(fused.spa()), sf)
        };

        // CombinedSweep takes the best attacking side; MixedSweep the harmonic mean of both
        let combined = |id: u32| {
            let (a, s, sf) = parts(id);
            a.max(s) * sf
        };
        let mixed = |id: u32| {
            let (a, s, sf) = parts(id);
            super::harmonic_mean(a, s) * sf
        };

        let asc = order_matches(
            &dex,
            matches.clone(),
            Some(Metric::CombinedSweep),
            None,
            StatMask::ALL,
        );
        assert_eq!(asc.iter().copied().collect::<RoaringBitmap>(), matches);
        assert!(asc.windows(2).all(|w| combined(w[0]) <= combined(w[1])));

        let asc_mixed = order_matches(
            &dex,
            matches.clone(),
            Some(Metric::MixedSweep),
            None,
            StatMask::ALL,
        );
        assert!(asc_mixed.windows(2).all(|w| mixed(w[0]) <= mixed(w[1])));
    }

    #[test]
    fn type_defense_rewards_resistant_typings() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let td = super::TypeDefense::build(dex.types());
        let ty = |key| dex.types().get_full_by_key(key).unwrap().0;
        let (normal, steel, flying, fire) = (ty("NORMAL"), ty("STEEL"), ty("FLYING"), ty("FIRE"));

        // a neutral-ish typing sits near 1; Steel (many resists + an immunity) is clearly bulkier
        assert!((td.factor(normal, None) - 1.0).abs() < 0.1);
        assert!(td.factor(steel, None) > 1.0);
        assert!(td.factor(steel, None) > td.factor(normal, None));
        // Steel/Flying improves on mono-Steel (Flying's Ground immunity erases Steel's weakness)
        assert!(td.factor(steel, Some(flying)) > td.factor(steel, None));
        // a typing that doubles a weakness (Steel/Fire is still weak to Fire & Ground/Fighting) is
        // worth less than the Steel/Flying combo
        assert!(td.factor(steel, Some(fire)) < td.factor(steel, Some(flying)));
        // ordering is symmetric in the two slots
        assert_eq!(
            td.factor(steel, Some(flying)),
            td.factor(flying, Some(steel))
        );
    }

    #[test]
    fn combined_bulk_scales_ehp_by_typing() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let n = dex.species().len() as u32;
        let bulbasaur = dex.species().get_id_of("BULBASAUR").unwrap();
        let td = super::TypeDefense::build(dex.types());

        let matches = Filters {
            has_pokemon: Some(HasPokemon::Either(bulbasaur)),
            ..Default::default()
        }
        .apply(&dex);

        let combined_bulk = |id: u32| {
            let head_sp = dex.species().get_item(SpeciesId::from_u32(id / n));
            let body_sp = dex.species().get_item(SpeciesId::from_u32(id % n));
            let fused = head_sp.base_stats.fuse(&body_sp.base_stats);
            let hp = f32::from(fused.hp());
            let ehp =
                super::harmonic_mean(hp * f32::from(fused.def()), hp * f32::from(fused.spd()));
            let (t1, t2) = super::fused_types(head_sp, body_sp, dex.types());
            ehp * td.factor(t1, t2)
        };

        let asc = order_matches(
            &dex,
            matches.clone(),
            Some(Metric::TACombinedEHp),
            None,
            StatMask::ALL,
        );
        // same set, just reordered, ordered by non-decreasing type-scaled bulk
        assert_eq!(asc.iter().copied().collect::<RoaringBitmap>(), matches);
        assert!(
            asc.windows(2)
                .all(|w| combined_bulk(w[0]) <= combined_bulk(w[1]))
        );
    }

    #[test]
    fn type_offense_rewards_strong_coverage() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let to = super::TypeOffense::build(dex.types());
        let ty = |key| dex.types().get_full_by_key(key).unwrap().0;
        let (normal, fighting, flying) = (ty("NORMAL"), ty("FIGHTING"), ty("FLYING"));

        // Normal has resists (Rock, Steel) and an immunity (Ghost) but nothing it hits super
        // effectively, so its average STAB coverage sits below the 1.0 neutral baseline
        assert!(to.factor(normal, None) < 1.0);
        // Fighting hits five types super effectively, so it out-covers Normal
        assert!(to.factor(fighting, None) > to.factor(normal, None));
        // a second STAB can only add coverage (per-defender max), so it never lowers the score and
        // Fighting's coverage strictly lifts mono-Normal
        assert!(to.factor(normal, Some(fighting)) > to.factor(normal, None));
        // the two slots are symmetric
        assert_eq!(
            to.factor(fighting, Some(flying)),
            to.factor(flying, Some(fighting))
        );
    }

    #[test]
    fn ta_combined_sweep_scales_sweep_by_typing() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let n = dex.species().len() as u32;
        let bulbasaur = dex.species().get_id_of("BULBASAUR").unwrap();
        let to = super::TypeOffense::build(dex.types());
        let curve = super::SpeedCurve::from_speed(dex.species().stat_distributions());

        let matches = Filters {
            has_pokemon: Some(HasPokemon::Either(bulbasaur)),
            ..Default::default()
        }
        .apply(&dex);

        // CombinedSweep (best attacking side × speed) scaled by the typing's offensive coverage,
        // recomputed independently of the metric code
        let ta_combined_sweep = |id: u32| {
            let head_sp = dex.species().get_item(SpeciesId::from_u32(id / n));
            let body_sp = dex.species().get_item(SpeciesId::from_u32(id % n));
            let fused = head_sp.base_stats.fuse(&body_sp.base_stats);
            let sweep = f32::from(fused.atk()).max(f32::from(fused.spa()))
                * curve.factor(f32::from(fused.spe()));
            let (t1, t2) = super::fused_types(head_sp, body_sp, dex.types());
            sweep * to.factor(t1, t2)
        };

        let asc = order_matches(
            &dex,
            matches.clone(),
            Some(Metric::TACombinedSweep),
            None,
            StatMask::ALL,
        );
        // same set, just reordered, ordered by non-decreasing type-scaled sweep
        assert_eq!(asc.iter().copied().collect::<RoaringBitmap>(), matches);
        assert!(
            asc.windows(2)
                .all(|w| ta_combined_sweep(w[0]) <= ta_combined_sweep(w[1]))
        );
    }

    #[test]
    fn synergy_variants_order_by_their_formulas() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let n = dex.species().len() as u32;
        let bulbasaur = dex.species().get_id_of("BULBASAUR").unwrap();

        let matches = Filters {
            has_pokemon: Some(HasPokemon::Either(bulbasaur)),
            ..Default::default()
        }
        .apply(&dex);

        // (fused, head, body) BSTs of a fusion
        let bsts = |id: u32| {
            let head = dex
                .species()
                .get_item(SpeciesId::from_u32(id / n))
                .base_stats;
            let body = dex
                .species()
                .get_item(SpeciesId::from_u32(id % n))
                .base_stats;
            (
                head.fuse(&body).bst() as f32,
                head.bst() as f32,
                body.bst() as f32,
            )
        };
        let ratio = |id| {
            let (f, h, b) = bsts(id);
            f / ((h + b) / 2.0)
        };
        let surplus_over_best = |id| {
            let (f, h, b) = bsts(id);
            f - h.max(b)
        };

        let asc_ratio = order_matches(
            &dex,
            matches.clone(),
            Some(Metric::SynergyRatio),
            None,
            StatMask::ALL,
        );
        assert_eq!(
            asc_ratio.iter().copied().collect::<RoaringBitmap>(),
            matches
        );
        assert!(asc_ratio.windows(2).all(|w| ratio(w[0]) <= ratio(w[1])));

        let asc_best = order_matches(
            &dex,
            matches.clone(),
            Some(Metric::SurplusOverBest),
            None,
            StatMask::ALL,
        );
        assert!(
            asc_best
                .windows(2)
                .all(|w| surplus_over_best(w[0]) <= surplus_over_best(w[1]))
        );
    }

    #[test]
    fn synergy_stat_mask_excludes_chosen_stats() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let n = dex.species().len() as u32;
        let bulbasaur = dex.species().get_id_of("BULBASAUR").unwrap();
        let matches = Filters {
            has_pokemon: Some(HasPokemon::Either(bulbasaur)),
            ..Default::default()
        }
        .apply(&dex);

        // SumOfParts with every stat but Sp. Atk: oracle sums the five included stats per side
        let included = [Stat::Hp, Stat::Atk, Stat::Def, Stat::Spd, Stat::Spe];
        let partial =
            |s: &super::BaseStats| -> f32 { included.iter().map(|&st| f32::from(s.get(st))).sum() };
        let sum_of_parts = |id: u32| {
            let head = dex
                .species()
                .get_item(SpeciesId::from_u32(id / n))
                .base_stats;
            let body = dex
                .species()
                .get_item(SpeciesId::from_u32(id % n))
                .base_stats;
            let f = partial(&head.fuse(&body));
            (f - partial(&head)) + (f - partial(&body))
        };

        let asc = order_matches(
            &dex,
            matches.clone(),
            Some(Metric::SumOfParts),
            None,
            StatMask::from_stats(&included),
        );
        assert_eq!(asc.iter().copied().collect::<RoaringBitmap>(), matches);
        assert!(
            asc.windows(2)
                .all(|w| sum_of_parts(w[0]) <= sum_of_parts(w[1]))
        );
        // an empty selection falls back to all stats
        assert!(matches!(StatMask::from_stats(&[]).0, 0b0011_1111));
    }

    #[test]
    fn balanced_synergy_orders_by_normalized_surplus() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let n = dex.species().len() as u32;
        let bulbasaur = dex.species().get_id_of("BULBASAUR").unwrap();
        let dist = dex.species().stat_distributions();
        let matches = Filters {
            has_pokemon: Some(HasPokemon::Either(bulbasaur)),
            ..Default::default()
        }
        .apply(&dex);

        let norm = |s: &super::BaseStats| -> f32 {
            Stat::VARIANTS
                .iter()
                .map(|&st| dist.rank(st, s.get(st)))
                .sum()
        };
        let balanced = |id: u32| {
            let head = dex
                .species()
                .get_item(SpeciesId::from_u32(id / n))
                .base_stats;
            let body = dex
                .species()
                .get_item(SpeciesId::from_u32(id % n))
                .base_stats;
            let f = norm(&head.fuse(&body));
            (f - norm(&head)) + (f - norm(&body))
        };

        let asc = order_matches(
            &dex,
            matches.clone(),
            Some(Metric::BalancedSynergy),
            None,
            StatMask::ALL,
        );
        assert_eq!(asc.iter().copied().collect::<RoaringBitmap>(), matches);
        assert!(asc.windows(2).all(|w| balanced(w[0]) <= balanced(w[1])));
    }

    #[test]
    fn ignored_species_drops_fusions_on_either_side() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let n = dex.species().len() as u32;
        let pikachu = dex.species().get_id_of("PIKACHU").unwrap();

        let kept = Filters {
            ignored_species: Box::from([pikachu]),
            ..Default::default()
        }
        .apply(&dex);

        // no surviving fusion has Pikachu on either side
        assert!(
            kept.iter()
                .all(|id| id / n != pikachu.to_u32() && id % n != pikachu.to_u32())
        );
        // and it really did remove some (every Pikachu row + column, minus the double-counted cell)
        let all = Filters::default().apply(&dex);
        assert_eq!(all.len() - kept.len(), 2 * u64::from(n) - 1);
    }
}
