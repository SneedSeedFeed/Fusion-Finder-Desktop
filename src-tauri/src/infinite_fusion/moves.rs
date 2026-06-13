use crate::infinite_fusion::moves::flags::MoveFlags;

pub mod flags;

pub struct MoveDex {}

pub struct Move {
    pub flags: MoveFlags,
    pub description: Box<str>,
}
