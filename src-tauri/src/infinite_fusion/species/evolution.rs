use serde::{Deserialize, Serialize};
use strum::{EnumString, VariantNames};

use crate::infinite_fusion::{
    Dex,
    items::{ItemDex, ItemId},
    moves::{MoveDex, MoveId},
    species::SpeciesId,
};

#[derive(Debug, Clone, Copy, Serialize)]
pub struct UnmappedEvolution<'a> {
    #[serde(borrow)]
    target: UnmappedEvolutionTarget<'a>,
    kind: EvolutionKind,
}

impl<'a> UnmappedEvolution<'a> {
    pub(crate) fn assign_id(self, id: SpeciesId) -> Evolution {
        let target = self.target.assign_id(id);
        Evolution {
            target,
            kind: self.kind,
        }
    }

    pub(crate) fn target(&self) -> &'a str {
        self.target.inner()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Evolution {
    target: EvolutionTarget,
    kind: EvolutionKind,
}

#[derive(Debug, Clone, Copy)]
pub struct EvolutionVisitor<'a> {
    pub(crate) move_dex: &'a MoveDex,
    pub(crate) item_dex: &'a ItemDex,
}

impl<'a, 'de> serde::de::DeserializeSeed<'de> for EvolutionVisitor<'a> {
    type Value = UnmappedEvolution<'de>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_seq(self)
    }
}

// Symbol("@evolutions"): Array([Array([Symbol(Symbol("CHARMELEON")), Symbol(Symbol("Level")), Integer(16), Bool(false)])]),
impl<'a, 'de> serde::de::Visitor<'de> for EvolutionVisitor<'a> {
    type Value = UnmappedEvolution<'de>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "infinite fusion's weird array evolution format thing"
        )
    }

    fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::SeqAccess<'de>,
    {
        let target = seq
            .next_element::<&'de str>()?
            .ok_or_else(|| serde::de::Error::custom("missing evolution target"))?;

        let kind_variant = seq
            .next_element::<EvolutionKindDiscriminants>()?
            .ok_or_else(|| serde::de::Error::custom("missing evolution kind"))?;

        let get_lvl = |a: &mut A| -> Result<u8, A::Error> {
            a.next_element::<u8>()
                .and_then(|s| s.ok_or_else(|| serde::de::Error::custom("missing level")))
        };

        let get_item = |a: &mut A| -> Result<ItemId, A::Error> {
            let item_text_id = a
                .next_element::<&str>()?
                .ok_or_else(|| serde::de::Error::custom("missing item id in data"))?;

            self.item_dex.get_id_of(item_text_id).ok_or_else(|| {
                serde::de::Error::custom(format_args!("{item_text_id} missing from ItemDex"))
            })
        };

        let get_move = |a: &mut A| -> Result<MoveId, A::Error> {
            let move_text_id = a
                .next_element::<&str>()?
                .ok_or_else(|| serde::de::Error::custom("missing move id in data"))?;

            self.move_dex.get_id_of(move_text_id).ok_or_else(|| {
                serde::de::Error::custom(format_args!("{move_text_id} missing from ItemDex"))
            })
        };
        let kind = match kind_variant {
            EvolutionKindDiscriminants::Level => EvolutionKind::Level {
                level: get_lvl(&mut seq)?,
            },
            EvolutionKindDiscriminants::LevelDay => EvolutionKind::LevelDay {
                level: get_lvl(&mut seq)?,
            },
            EvolutionKindDiscriminants::LevelNight => EvolutionKind::LevelNight {
                level: get_lvl(&mut seq)?,
            },
            EvolutionKindDiscriminants::AttackGreater => EvolutionKind::AttackGreater {
                level: get_lvl(&mut seq)?,
            },
            EvolutionKindDiscriminants::DefenseGreater => EvolutionKind::DefenseGreater {
                level: get_lvl(&mut seq)?,
            },
            EvolutionKindDiscriminants::AtkDefEqual => EvolutionKind::AtkDefEqual {
                level: get_lvl(&mut seq)?,
            },
            EvolutionKindDiscriminants::Shedinja => EvolutionKind::Shedinja {
                level: get_lvl(&mut seq)?,
            },
            EvolutionKindDiscriminants::Ninjask => EvolutionKind::Ninjask {
                level: get_lvl(&mut seq)?,
            },
            EvolutionKindDiscriminants::Silcoon => EvolutionKind::Silcoon {
                level: get_lvl(&mut seq)?,
            },
            EvolutionKindDiscriminants::Cascoon => EvolutionKind::Cascoon {
                level: get_lvl(&mut seq)?,
            },
            EvolutionKindDiscriminants::Item => EvolutionKind::Item {
                item_id: get_item(&mut seq)?,
            },
            EvolutionKindDiscriminants::DayHoldItem => EvolutionKind::DayHoldItem {
                item_id: get_item(&mut seq)?,
            },
            EvolutionKindDiscriminants::TradeItem => EvolutionKind::TradeItem {
                item_id: get_item(&mut seq)?,
            },
            EvolutionKindDiscriminants::HasMove => EvolutionKind::HasMove {
                move_id: get_move(&mut seq)?,
            },
        };

        let is_devolution = seq
            .next_element::<bool>()?
            .ok_or_else(|| serde::de::Error::custom("missing is_devolution flag"))?;

        let unmapped_target = match is_devolution {
            true => UnmappedEvolutionTarget::From(target),
            false => UnmappedEvolutionTarget::Into(target),
        };

        Ok(Self::Value {
            target: unmapped_target,
            kind,
        })
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    Deserialize,
    Serialize,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    strum::EnumDiscriminants,
)]
#[strum_discriminants[derive(EnumString, VariantNames, Deserialize, Serialize)]]
#[serde(tag = "type", deny_unknown_fields)]
pub enum EvolutionKind {
    Level { level: u8 },
    Item { item_id: ItemId },
    AttackGreater { level: u8 },
    DefenseGreater { level: u8 },
    HasMove { move_id: MoveId },
    AtkDefEqual { level: u8 },
    DayHoldItem { item_id: ItemId },
    Shedinja { level: u8 },
    Ninjask { level: u8 },
    LevelDay { level: u8 },
    LevelNight { level: u8 },
    Silcoon { level: u8 },
    Cascoon { level: u8 },
    TradeItem { item_id: ItemId },
}

#[derive(Debug, Copy, Clone, Serialize)]
pub enum UnmappedEvolutionTarget<'a> {
    From(&'a str),
    Into(&'a str),
}

impl<'a> UnmappedEvolutionTarget<'a> {
    pub fn inner(&self) -> &'a str {
        match self {
            UnmappedEvolutionTarget::From(x) => x,
            UnmappedEvolutionTarget::Into(x) => x,
        }
    }

    pub(crate) fn assign_id(self, id: SpeciesId) -> EvolutionTarget {
        match self {
            UnmappedEvolutionTarget::From(_) => EvolutionTarget::From { target: id },
            UnmappedEvolutionTarget::Into(_) => EvolutionTarget::Into { target: id },
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum EvolutionTarget {
    From { target: SpeciesId },
    Into { target: SpeciesId },
}
