use serde::{Deserialize, Serialize};

use crate::infinite_fusion::{
    Dex,
    moves::{MoveDex, MoveId},
};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct LevelMove {
    move_id: MoveId,
    level: u8,
}

impl LevelMove {
    pub fn level(&self) -> u8 {
        self.level
    }
    pub fn move_id(&self) -> MoveId {
        self.move_id
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct LevelMoveVisitor<'a>(pub(crate) &'a MoveDex);

impl<'de, 'a> serde::de::DeserializeSeed<'de> for LevelMoveVisitor<'a> {
    type Value = LevelMove;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        <(u8, &str) as Deserialize>::deserialize(deserializer).and_then(|(level, move_key)| {
            self.0
                .get_id_of(move_key)
                .map(|move_id| LevelMove { move_id, level })
                .ok_or_else(|| {
                    serde::de::Error::custom(format_args!("Move {move_key} not found in MoveDex"))
                })
        })
    }
}
