pub mod ability_filter;
pub mod custom_sprite_filter;
pub mod move_filter;
pub mod stat_filter;
pub mod type_filter;

use roaring::RoaringBitmap;
use serde::{Deserialize, Serialize};

use crate::infinite_fusion::{
    Dex, DexId, InfiniteFusionDex, abilities::AbilityId, moves::MoveId, species::SpeciesId,
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
    pub has_pokemon: Option<SpeciesId>,
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
    /// exclude species whose in-game dex number exceeds this (hidden, game-set: Kanto's data
    /// carries Gen-3 species it can't actually fuse, so we cap at the real fusable count)
    #[serde(default)]
    pub block_ids_above: Option<u16>,
}

impl Filters {
    /// One per-head pass: intersect every active filter's per-head body set, emit the matches.
    /// Done this way so a broad filter never builds its full fusion-id set it just hands over a per-head species bitmap, so cost tracks the *narrowest* filter, not the largest.
    pub fn apply(&self, dex: &InfiniteFusionDex) -> RoaringBitmap {
        let n = dex.species().len();
        let mut result = RoaringBitmap::new();

        // species (by index) allowed under the id cap; both head and body must be allowed
        let allowed: Option<RoaringBitmap> = self.block_ids_above.map(|max| {
            dex.species()
                .map()
                .values()
                .enumerate()
                .filter_map(|(i, s)| (s.id_number <= max).then_some(i as u32))
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

            // body must be the chosen species (unless the head already is it)
            if let Some(pokemon) = self.has_pokemon
                && head != pokemon
            {
                let mut only = RoaringBitmap::new();
                only.insert(pokemon.to_u32());
                and_in(&mut bodies, only);
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

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
pub enum SortBy {
    #[default]
    DexNumber,
    Hp,
    Atk,
    Def,
    Spa,
    Spd,
    Spe,
    Bst,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, Default)]
pub enum SortOrder {
    #[default]
    Ascending,
    Descending,
}

impl SortOrder {
    pub fn is_descending(self) -> bool {
        self == SortOrder::Descending
    }

    pub fn is_ascending(self) -> bool {
        self == SortOrder::Ascending
    }
}

impl SortBy {
    /// Order the matching fusion ids. `DexNumber` uses the fusion id itself; any stat uses the
    /// *fused* value. `descending` flips the order; ties always fall back to ascending id.
    pub fn order(
        self,
        dex: &InfiniteFusionDex,
        matches: RoaringBitmap,
        order: SortOrder,
    ) -> Vec<u32> {
        if self == SortBy::DexNumber {
            // RoaringBitmap iterates ascending
            let mut ids: Vec<u32> = matches.iter().collect();
            if order.is_descending() {
                ids.reverse();
            }
            return ids;
        }
        let n = dex.species().len() as u32;
        let mut keyed: Vec<(u16, u32)> = matches
            .iter()
            .map(|id| (self.fused_stat(dex, id / n, id % n), id))
            .collect();
        keyed.sort_by(|a, b| {
            let by_stat = if order.is_descending() {
                b.0.cmp(&a.0)
            } else {
                a.0.cmp(&b.0)
            };
            by_stat.then(a.1.cmp(&b.1))
        });
        keyed.into_iter().map(|(_, id)| id).collect()
    }

    fn fused_stat(self, dex: &InfiniteFusionDex, head: u32, body: u32) -> u16 {
        let head = dex.species().get_item(SpeciesId::from_u32(head)).base_stats;
        let body = dex.species().get_item(SpeciesId::from_u32(body)).base_stats;
        let fused = head.fuse(&body);
        match self {
            SortBy::Hp => fused.hp().into(),
            SortBy::Atk => fused.atk().into(),
            SortBy::Def => fused.def().into(),
            SortBy::Spa => fused.spa().into(),
            SortBy::Spd => fused.spd().into(),
            SortBy::Spe => fused.spe().into(),
            SortBy::Bst => fused.bst(),
            SortBy::DexNumber => 0,
        }
    }
}

/// Everything the front end needs on open to populate its filter controls.
#[derive(Debug, Clone, Serialize)]
pub struct FilterOptions {
    /// a fusion id is `head * species_count + body`; the front end needs this to decode results
    pub species_count: usize,
    pub species: Vec<SpeciesOption>,
    pub moves: Vec<NamedId>,
    pub types: Vec<NamedId>,
    pub abilities: Vec<NamedId>,
    pub stat_bounds: StatBounds,
    /// default value for the hidden id-cap filter (`block_ids_above`); `None` = no cap
    pub block_ids_above: Option<u16>,
}

/// A dex entry's id and its display name.
#[derive(Debug, Clone, Serialize)]
pub struct NamedId {
    pub id: u32,
    pub name: String,
}

/// A species' id, its real display name, plus its name halves
#[derive(Debug, Clone, Serialize)]
pub struct SpeciesOption {
    pub id: u32,
    /// in-game dex number (`id_number`); used to build sprite URLs, distinct from `id` (our index)
    pub dex_id: u16,
    pub name: String,
    pub first: String,
    pub second: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct StatBounds {
    pub hp: StatRange<u8>,
    pub atk: StatRange<u8>,
    pub def: StatRange<u8>,
    pub spa: StatRange<u8>,
    pub spd: StatRange<u8>,
    pub spe: StatRange<u8>,
    pub bst: StatRange<u16>,
}

/// `(id, name)` for every entry of a dex, in id order.
pub(crate) fn named_ids<D: Dex>(dex: &D, name: impl Fn(&D::Item) -> String) -> Vec<NamedId> {
    dex.map()
        .values()
        .enumerate()
        .map(|(i, item)| NamedId {
            id: D::Id::from_usize(i).to_u32(),
            name: name(item),
        })
        .collect()
}

#[cfg(test)]
mod test {
    use roaring::RoaringBitmap;

    use super::{
        Filters, HasAbility, HasMove, SortBy, StatRange, StatRanges, ability_filter::AbilitySource,
        move_filter::MoveSource, separable_filter, stat_filter::FusedStat,
        stat_filter::StatRange as TaggedRange,
    };
    use crate::{
        infinite_fusion::{
            Dex, DexId, GameVersion, InfiniteFusionDex, filters::SortOrder, species::SpeciesId,
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
            has_custom_sprite: false,
            block_ids_above: None,
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
            has_pokemon: Some(bulbasaur),
            has_type: Box::default(),
            stat_range: StatRanges::default(),
            has_ability: None,
            has_move: None,
            has_custom_sprite: false,
            block_ids_above: None,
        };

        let mut only = RoaringBitmap::new();
        only.insert(bulbasaur.to_u32());
        let naive = separable_filter(n, &only);

        let optimised = filters.apply(&dex);
        assert!(!optimised.is_empty());
        assert_eq!(optimised, naive);
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
    fn sort_by_stat_is_descending_and_lossless() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let n = dex.species().len() as u32;
        let bulbasaur = dex.species().get_id_of("BULBASAUR").unwrap();

        let matches = Filters {
            has_pokemon: Some(bulbasaur),
            ..Default::default()
        }
        .apply(&dex);

        let ordered = SortBy::Bst.order(&dex, matches.clone(), SortOrder::Descending);
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
        // descending: non-increasing fused BST
        assert!(ordered.windows(2).all(|w| bst(w[0]) >= bst(w[1])));
        // ascending: non-decreasing fused BST
        let asc = SortBy::Bst.order(&dex, matches.clone(), SortOrder::Ascending);
        assert!(asc.windows(2).all(|w| bst(w[0]) <= bst(w[1])));
        // DexNumber: ascending vs descending id order
        assert!(
            SortBy::DexNumber
                .order(&dex, matches.clone(), SortOrder::Ascending)
                .windows(2)
                .all(|w| w[0] < w[1])
        );
        assert!(
            SortBy::DexNumber
                .order(&dex, matches, SortOrder::Descending)
                .windows(2)
                .all(|w| w[0] > w[1])
        );
    }
}
