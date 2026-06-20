use std::num::NonZeroU8;

use serde::Serialize;

use crate::infinite_fusion::{
    Dex, DexId,
    abilities::AbilityId,
    filters::StatRange,
    moves::{Accuracy, MoveCategory, MoveId, flags::MoveFlags},
    species::SpeciesId,
    types::TypeId,
};

/// everything the frontend needs on opening
#[derive(Debug, Clone, Serialize)]
pub struct Bootstrap {
    /// a fusion id is `head * species_count + body`; the front end needs this to decode results
    pub species_count: usize,
    pub species: Vec<SpeciesOption>,
    pub moves: Vec<MoveOption>,
    pub types: Vec<NamedId<TypeId>>,
    pub abilities: Vec<NamedId<AbilityId>>,
    pub stat_bounds: StatBounds,
    /// slider bounds for the move-list
    pub move_power: StatRange<u8>,
    pub move_effect_chance: StatRange<u8>,
    pub move_accuracy: StatRange<u8>,
    pub move_priority: StatRange<i8>,
    /// default value for the hidden id-cap filter (`block_ids_above`) where `None` = no cap
    pub block_ids_above: Option<u16>,
}

/// A dex entry's id and its display name.
#[derive(Debug, Clone, Serialize)]
pub struct NamedId<T> {
    pub id: T,
    pub name: Box<str>,
}

/// A move plus the properties the front end filters/sorts the (large) move list by.
#[derive(Debug, Clone, Serialize)]
pub struct MoveOption {
    pub id: MoveId,
    pub name: Box<str>,
    /// the move's type; resolve its name/icon via the `types` table
    pub ty: TypeId,
    /// "Physical" / 0 | "Special" / 1 | "Status" / 2
    pub category: MoveCategory,
    pub power: Option<NonZeroU8>,
    pub effect_chance: Option<NonZeroU8>,
    pub accuracy: Accuracy,
    pub priority: i8,
    pub description: Box<str>,
    pub flags: MoveFlags,
}

/// A species' id, its real display name, plus its name halves
#[derive(Debug, Clone, Serialize)]
pub struct SpeciesOption {
    pub id: SpeciesId,
    /// in-game dex number (`id_number`); used to build sprite URLs, distinct from `id` (our index)
    pub dex_id: u16,
    pub name: Box<str>,
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
pub(crate) fn named_ids<D: Dex>(
    dex: &D,
    name: impl Fn(&D::Item) -> Box<str>,
) -> Vec<NamedId<D::Id>> {
    dex.map()
        .values()
        .enumerate()
        .map(|(i, item)| NamedId {
            id: D::Id::from_usize(i),
            name: name(item),
        })
        .collect()
}
