macro_rules! create_base_stats {
    ($name:ident, $($stat:ident => $key:literal),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
        pub struct $name {
            $(#[serde(rename = $key)] $stat: u8,)*
        }

        impl $name {
            $(pub fn $stat(&self) -> u8 {
                self.$stat
            })*
        }
    };
}

create_base_stats!(
    BaseStats,
    hp => "HP",
    atk => "ATTACK",
    def => "DEFENSE",
    spa => "SPECIAL_ATTACK",
    spd => "SPECIAL_DEFENSE",
    spe => "SPEED",
);

impl BaseStats {
    pub fn fuse(&self, body: &Self) -> Self {
        // Head dominant
        let hp = fuse_calc(self.hp, body.hp);
        let spd = fuse_calc(self.spd, body.spd);
        let spa = fuse_calc(self.spa, body.spa);

        // Body dominant
        let atk = fuse_calc(body.atk, self.atk);
        let def = fuse_calc(body.def, self.def);
        let spe = fuse_calc(body.spe, self.spe);
        Self {
            hp,
            atk,
            def,
            spa,
            spd,
            spe,
        }
    }

    pub fn bst(&self) -> u16 {
        self.hp as u16
            + self.atk as u16
            + self.def as u16
            + self.spa as u16
            + self.spd as u16
            + self.spe as u16
    }
}

fn fuse_calc(dominant: u8, other: u8) -> u8 {
    (((dominant as f32 * 2.0) / 3.0) + (other as f32 / 3.0)).floor() as u8
}
