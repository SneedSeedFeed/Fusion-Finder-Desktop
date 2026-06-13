use bitflags::bitflags;
use serde::Deserialize;

bitflags! {
    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
    pub struct MoveFlags: u16 {
        const CONTACT = 1 << 0;
        const CAN_PROTECT = 1 << 1;
        const CAN_MAGIC_COAT = 1 << 2;
        const CAN_SNATCH = 1 << 3;
        const CAN_MIRROR_MOVE = 1 << 4;
        const CAN_KINGS_ROCK = 1 << 5;
        const THAWS = 1 << 6;
        const HIGH_CRIT_RATE = 1 << 7;
        const BITING = 1 << 8;
        const PUNCHING = 1 << 9;
        const SOUND = 1 << 10;
        const POWDER = 1 << 11;
        const PULSE = 1 << 12;
        const BOMB = 1 << 13;
        const DANCE = 1 << 14;
    }
}

impl<'de> serde::Deserialize<'de> for MoveFlags {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = <&str as Deserialize<'de>>::deserialize(deserializer)?;
        let mut flags = MoveFlags::default();
        s.bytes().try_for_each(|b| {
            match b {
                b'a' => flags.set(MoveFlags::CONTACT, true),
                b'b' => flags.set(MoveFlags::CAN_PROTECT, true),
                b'c' => flags.set(MoveFlags::CAN_MAGIC_COAT, true),
                b'd' => flags.set(MoveFlags::CAN_SNATCH, true),
                b'e' => flags.set(MoveFlags::CAN_MIRROR_MOVE, true),
                b'f' => flags.set(MoveFlags::CAN_KINGS_ROCK, true),
                b'g' => flags.set(MoveFlags::THAWS, true),
                b'h' => flags.set(MoveFlags::HIGH_CRIT_RATE, true),
                b'i' => flags.set(MoveFlags::BITING, true),
                b'j' => flags.set(MoveFlags::PUNCHING, true),
                b'k' => flags.set(MoveFlags::SOUND, true),
                b'l' => flags.set(MoveFlags::POWDER, true),
                b'm' => flags.set(MoveFlags::PULSE, true),
                b'n' => flags.set(MoveFlags::BOMB, true),
                b'o' => flags.set(MoveFlags::DANCE, true),
                other => {
                    return Err(serde::de::Error::custom(format_args!(
                        "invalid move byte flag '{other}'"
                    )));
                }
            };
            Ok(())
        })?;
        Ok(flags)
    }
}
// resisting every urge in my body not to macro this up for no reason
impl serde::Serialize for MoveFlags {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut s = String::new();
        if self.contains(MoveFlags::CONTACT) {
            s.push('a');
        }
        if self.contains(MoveFlags::CAN_PROTECT) {
            s.push('b');
        }
        if self.contains(MoveFlags::CAN_MAGIC_COAT) {
            s.push('c');
        }
        if self.contains(MoveFlags::CAN_SNATCH) {
            s.push('d');
        }
        if self.contains(MoveFlags::CAN_MIRROR_MOVE) {
            s.push('e');
        }
        if self.contains(MoveFlags::CAN_KINGS_ROCK) {
            s.push('f');
        }
        if self.contains(MoveFlags::THAWS) {
            s.push('g');
        }
        if self.contains(MoveFlags::HIGH_CRIT_RATE) {
            s.push('h');
        }
        if self.contains(MoveFlags::BITING) {
            s.push('i');
        }
        if self.contains(MoveFlags::PUNCHING) {
            s.push('j');
        }
        if self.contains(MoveFlags::SOUND) {
            s.push('k');
        }
        if self.contains(MoveFlags::POWDER) {
            s.push('l');
        }
        if self.contains(MoveFlags::PULSE) {
            s.push('m');
        }
        if self.contains(MoveFlags::BOMB) {
            s.push('n');
        }
        if self.contains(MoveFlags::DANCE) {
            s.push('o');
        }
        serializer.serialize_str(&s)
    }
}
