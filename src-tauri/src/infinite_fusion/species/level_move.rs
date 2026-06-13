use serde::Deserialize;

use crate::infinite_fusion::{
    Dex,
    moves::{MoveDex, MoveId},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, Hash)]
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
        let (level, move_key) = <(u8, &str) as Deserialize>::deserialize(deserializer)?;

        match self.0.get_id_of(move_key) {
            Some(move_id) => Ok(LevelMove { move_id, level }),
            None => todo!(),
        }
    }
}
