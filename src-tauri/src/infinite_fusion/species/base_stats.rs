macro_rules! create_base_stats {
    ($name:ident, $($stat:ident => $key:literal),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialOrd, Ord, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
        pub struct $name {
            $(#[serde(rename = $key)] $stat: u8,)*
        }

        impl $name {
            pub const MIN: $name = $name {
                $($stat: u8::MIN),*
            };

            pub const MAX: $name = $name {
                $($stat: u8::MAX),*
            };

            $(pub fn $stat(&self) -> u8 {
                self.$stat
            })*

            pub fn apply_min(&mut self, other: &Self) {
                $(self.$stat = self.$stat.min(other.$stat);)*
            }

            pub fn apply_max(&mut self, other: &Self) {
                $(self.$stat = self.$stat.max(other.$stat);)*
            }
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
    (2 * u16::from(dominant) / 3 + u16::from(other) / 3) as u8
}
