use strum::VariantArray;

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
    ((2 * u16::from(dominant) + u16::from(other)) / 3) as u8
}

/// One of the six base stats, used to index per-stat tables like [`StatDistributions`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::VariantArray)]
pub enum Stat {
    Hp = 0,
    Atk = 1,
    Def = 2,
    Spa = 3,
    Spd = 4,
    Spe = 5,
}

impl BaseStats {
    pub fn get(&self, stat: Stat) -> u8 {
        match stat {
            Stat::Hp => self.hp,
            Stat::Atk => self.atk,
            Stat::Def => self.def,
            Stat::Spa => self.spa,
            Stat::Spd => self.spd,
            Stat::Spe => self.spe,
        }
    }
}

/// Per-stat histograms of every species' base stats
#[derive(Debug, Clone)]
pub struct StatDistributions {
    histograms: Box<[[u16; 256]; 6]>, // indexed by `stat as usize` and i boxed it because I felt having such a big type on the stack was too much
    count: u16,
}

impl Default for StatDistributions {
    fn default() -> Self {
        Self {
            histograms: Box::from([[0; 256]; 6]),
            count: 0,
        }
    }
}

impl StatDistributions {
    /// Fold one species' base stats into the histograms
    pub fn record(&mut self, stats: &BaseStats) {
        for &stat in Stat::VARIANTS {
            self.histograms[stat as usize][usize::from(stats.get(stat))] += 1;
        }
        self.count += 1;
    }

    pub fn count(&self) -> u16 {
        self.count
    }

    /// The `q`-quantile (`0..=1`) of `stat` across all recorded species: the smallest base value at
    /// or below which at least a `q` fraction of species fall. An empty distribution yields 0.
    pub fn percentile(&self, stat: Stat, q: f32) -> u8 {
        if self.count == 0 {
            return 0;
        }
        let q = q.clamp(0.0, 1.0);
        let target = ((q * self.count as f32).ceil() as u16).max(1);
        let mut cumulative = 0;
        for (value, &count) in self.histograms[stat as usize].iter().enumerate() {
            cumulative += count;
            if cumulative >= target {
                return value as u8;
            }
        }
        u8::MAX
    }
}

#[cfg(test)]
mod test {
    use super::{BaseStats, Stat, StatDistributions};

    fn with_speed(spe: u8) -> BaseStats {
        BaseStats {
            hp: 0,
            atk: 0,
            def: 0,
            spa: 0,
            spd: 0,
            spe,
        }
    }

    #[test]
    fn percentile_reads_off_the_histogram() {
        let mut dist = StatDistributions::default();
        // speeds 10, 20, 30, …, 100 - one species each
        for s in 1..=10 {
            dist.record(&with_speed(s * 10));
        }
        assert_eq!(dist.count(), 10);

        // q=0 -> the minimum, q=1 -> the maximum
        assert_eq!(dist.percentile(Stat::Spe, 0.0), 10);
        assert_eq!(dist.percentile(Stat::Spe, 1.0), 100);
        // median: ceil(0.5·10)=5th value = 50
        assert_eq!(dist.percentile(Stat::Spe, 0.5), 50);
        // p90: ceil(0.9·10)=9th value = 90
        assert_eq!(dist.percentile(Stat::Spe, 0.9), 90);
        // a stat we never recorded a nonzero value for is all-zero
        assert_eq!(dist.percentile(Stat::Atk, 0.5), 0);
        // an empty distribution yields 0
        assert_eq!(StatDistributions::default().percentile(Stat::Spe, 0.5), 0);
    }

    #[test]
    fn fuse_rounds_the_combined_numerator() {
        let dhelmise = BaseStats {
            hp: 70,
            atk: 131,
            def: 100,
            spa: 86,
            spd: 90,
            spe: 40,
        };
        let kyogre = BaseStats {
            hp: 100,
            atk: 100,
            def: 90,
            spa: 150,
            spd: 140,
            spe: 90,
        };

        let fused = dhelmise.fuse(&kyogre);
        assert_eq!(
            (
                fused.hp(),
                fused.atk(),
                fused.def(),
                fused.spa(),
                fused.spd(),
                fused.spe()
            ),
            (80, 110, 93, 107, 106, 73),
        );
        assert_eq!(fused.bst(), 569);
    }
}
