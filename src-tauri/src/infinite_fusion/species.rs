use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::{
    dex_id,
    infinite_fusion::{
        abilities::AbilityId,
        moves::MoveId,
        species::{
            base_stats::BaseStats,
            evolution::{Evolution, UnmappedEvolution},
            level_move::LevelMove,
            name_halves::NameHalves,
        },
        types::TypeId,
    },
};

pub mod base_stats;
pub mod evolution;
pub mod level_move;
pub mod name_halves;

#[derive(Debug, Clone)]
pub struct SpeciesDex {
    map: IndexMap<Box<str>, SpeciesDetails>,
}

dex_id!(SpeciesId, u16);

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct SpeciesDetails {
    pub(crate) id_number: u16,
    pub(crate) names: NameHalves,
    pub(crate) type1: TypeId,
    pub(crate) type2: Option<TypeId>,
    pub(crate) base_stats: BaseStats,
    pub(crate) moves: Box<[LevelMove]>,
    pub(crate) tutor_moves: Box<[MoveId]>,
    pub(crate) egg_moves: Box<[MoveId]>,
    pub(crate) abilities: Box<[AbilityId]>,
    pub(crate) hidden_abilities: Box<[AbilityId]>,
    pub(crate) evolutions: Box<[Evolution]>,
}

pub struct UnmappedSpeciesDetails<'a> {
    id_number: u16,
    names: NameHalves,
    type1: TypeId,
    type2: Option<TypeId>,
    base_stats: BaseStats,
    moves: Box<[LevelMove]>,
    tutor_moves: Box<[MoveId]>,
    egg_moves: Box<[MoveId]>,
    abilities: Box<[AbilityId]>,
    hidden_abilities: Box<[AbilityId]>,
    evolutions: Box<[UnmappedEvolution<'a>]>,
}
