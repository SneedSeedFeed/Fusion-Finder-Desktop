pub mod ability_filter;
pub mod custom_sprite_filter;
pub mod move_filter;
pub mod stat_filter;
pub mod type_filter;

use roaring::RoaringBitmap;
use serde::{Deserialize, Serialize};

use crate::infinite_fusion::{
    Dex, DexId, InfiniteFusionDex,
    abilities::AbilityId,
    filters::type_filter::fused_types,
    moves::MoveId,
    species::{
        SpeciesDetails, SpeciesId,
        base_stats::{Stat, StatDistributions},
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

        // species (by index) allowed by the id cap and/or the legendary exclusion; both head and
        // body must be allowed. Built once, then intersected per-head like any other constraint.
        let allowed: Option<RoaringBitmap> =
            (self.block_ids_above.is_some() || self.exclude_legendaries).then(|| {
                let legendaries = dex.legendaries();
                dex.species()
                    .map()
                    .values()
                    .enumerate()
                    .filter_map(|(i, s)| {
                        let under_cap = self.block_ids_above.is_none_or(|max| s.id_number <= max);
                        let not_legendary =
                            !self.exclude_legendaries || !legendaries.contains(i as u32);
                        (under_cap && not_legendary).then_some(i as u32)
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
    // effective health metrics
    PhysicalEhp,
    SpecialEhp,
    /// harmonic mean of Physical and Special effective health to reward generally bulky pokemon
    CombinedEhp,
    // sweep metrics: an attacking stat scaled by how usable its speed is
    PhysicalSweep,
    SpecialSweep,
    // the better attacking side scaled by speed
    CombinedSweep,
    // the harmonic mean of both attacking sides scaled by speed (rewards mixed attackers like salamence)
    MixedSweep,
}

/// Harmonic mean of two values. Pulled toward the smaller of the two, so a lopsided pair scores far
/// below a balanced one — the point of "combined" bulk.
fn harmonic_mean(a: f32, b: f32) -> f32 {
    let sum = a + b;
    if sum == 0.0 {
        return 0.0;
    }
    2.0 * a * b / sum
}

/// Logistic mapping from speed to its estimated offensive usefulness.
/// Mapping is calibrated to the dex's own speed distribution.
/// Centred on the 50th percentile and tapers at p10 and p90, so a handful of speed freaks can't stretch it.
/// Is intended to reward attackers at "efficient" speed stats like Garchomp who have enough speed to outspeed and kill but don't waste it like Regieleki
#[derive(Debug, Clone, Copy)]
struct SpeedCurve {
    midpoint: f32, // median speed -> factor 0.5
    steepness: f32,
}

impl SpeedCurve {
    fn from_speed(dist: &StatDistributions) -> Self {
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

    fn factor(&self, spe: f32) -> f32 {
        1.0 / (1.0 + (-self.steepness * (spe - self.midpoint)).exp())
    }
}

/// Per-search evaluation context for metrics: the dex plus any distribution-derived calibration,
/// built once so the hot per-fusion loop stays O(1). Future metrics needing other distribution stats
/// add fields here rather than recomputing per fusion.
struct MetricContext<'a> {
    dex: &'a InfiniteFusionDex,
    speed: SpeedCurve,
}

impl<'a> MetricContext<'a> {
    fn new(dex: &'a InfiniteFusionDex) -> Self {
        Self {
            dex,
            speed: SpeedCurve::from_speed(dex.species().stat_distributions()),
        }
    }
}

impl Metric {
    fn fused_value(self, ctx: &MetricContext, head: u32, body: u32) -> f32 {
        let dex = ctx.dex;
        let head = dex.species().get_item(SpeciesId::from_u32(head)).base_stats;
        let body = dex.species().get_item(SpeciesId::from_u32(body)).base_stats;
        let fused = head.fuse(&body);
        let hp = f32::from(fused.hp());
        let (atk, spa) = (f32::from(fused.atk()), f32::from(fused.spa()));
        let sweep_speed = || ctx.speed.factor(f32::from(fused.spe()));
        match self {
            Metric::Hp => hp,
            Metric::Atk => atk,
            Metric::Def => fused.def().into(),
            Metric::Spa => spa,
            Metric::Spd => fused.spd().into(),
            Metric::Spe => fused.spe().into(),
            Metric::Bst => fused.bst().into(),
            Metric::PhysicalEhp => hp * f32::from(fused.def()),
            Metric::SpecialEhp => hp * f32::from(fused.spd()),
            Metric::CombinedEhp => {
                harmonic_mean(hp * f32::from(fused.def()), hp * f32::from(fused.spd()))
            }
            Metric::PhysicalSweep => atk * sweep_speed(),
            Metric::SpecialSweep => spa * sweep_speed(),
            Metric::CombinedSweep => atk.max(spa) * sweep_speed(),
            Metric::MixedSweep => harmonic_mean(atk, spa) * sweep_speed(),
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
) -> Vec<u32> {
    let Some(primary) = metric else {
        // RoaringBitmap iterates ascending, which is exactly dex order
        return matches.iter().collect();
    };
    let n = dex.species().len() as u32;
    let ctx = MetricContext::new(dex); // calibrate the speed tiering curve

    // one f32 key per fusion: the metric itself, or the metric/secondary ratio
    let mut keyed: Vec<(f32, u32)> = matches
        .iter()
        .map(|id| {
            let value = primary.fused_value(&ctx, id / n, id % n);
            let key = match secondary {
                Some(denom) => value / denom.fused_value(&ctx, id / n, id % n),
                None => value,
            };
            (key, id)
        })
        .collect();
    keyed.sort_unstable_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    keyed.into_iter().map(|(_, id)| id).collect()
}

#[cfg(test)]
mod test {
    use roaring::RoaringBitmap;

    use super::{
        DefenseFilter, DefenseRelation, EvolutionFilter, Filters, HasAbility, HasMove, HasPokemon,
        Metric, StatRange, StatRanges, ability_filter::AbilitySource, move_filter::MoveSource,
        order_matches, separable_filter, stat_filter::FusedStat,
        stat_filter::StatRange as TaggedRange,
    };
    use crate::{
        infinite_fusion::{Dex, DexId, GameVersion, InfiniteFusionDex, species::SpeciesId},
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

        let ordered = order_matches(&dex, matches.clone(), Some(Metric::Bst), None);
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
            order_matches(&dex, matches.clone(), None, None)
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

        let asc = order_matches(&dex, matches.clone(), Some(Metric::Atk), Some(Metric::Def));
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

        let asc = order_matches(&dex, matches.clone(), Some(Metric::CombinedEhp), None);
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

        let asc = order_matches(&dex, matches.clone(), Some(Metric::CombinedSweep), None);
        assert_eq!(asc.iter().copied().collect::<RoaringBitmap>(), matches);
        assert!(asc.windows(2).all(|w| combined(w[0]) <= combined(w[1])));

        let asc_mixed = order_matches(&dex, matches.clone(), Some(Metric::MixedSweep), None);
        assert!(asc_mixed.windows(2).all(|w| mixed(w[0]) <= mixed(w[1])));
    }
}
