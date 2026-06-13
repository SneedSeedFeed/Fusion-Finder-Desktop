use std::{
    fmt::Display,
    num::{NonZero, NonZeroU8},
    path::Path,
};

use indexmap::IndexMap;
use reikland::MixedKeyRef;
use serde::{
    Deserialize, Serialize,
    de::{DeserializeSeed, IgnoredAny, Unexpected, Visitor},
};

use crate::{
    dex_id,
    infinite_fusion::{
        Dex,
        moves::flags::MoveFlags,
        types::{TypeDex, TypeId},
    },
};

pub mod flags;

pub struct MoveDex {
    map: IndexMap<Box<str>, MoveDetails>,
}

impl Dex for MoveDex {
    fn relative_path() -> &'static Path {
        Path::new("Data/moves.dat")
    }

    type Id = MoveId;

    type Item = MoveDetails;

    fn map(&self) -> &IndexMap<Box<str>, Self::Item> {
        &self.map
    }
}

dex_id!(MoveId, u16);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MoveDetails {
    pub name: Box<str>,
    pub function_code: Box<str>,
    pub power: Option<NonZeroU8>,
    pub ty: TypeId,
    pub category: MoveCategory,
    pub accuracy: Accuracy,
    pub pp: u8,
    pub effect_chance: Option<NonZeroU8>,
    pub target: MoveTarget,
    pub priority: i8,
    pub flags: MoveFlags,
    pub description: Box<str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Accuracy {
    Percent(NonZeroU8),
    Always,
}

impl<'de> Deserialize<'de> for Accuracy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match u8::deserialize(deserializer)? {
            0 => Ok(Accuracy::Always),
            percent => Ok(Accuracy::Percent(NonZero::new(percent).unwrap())),
        }
    }
}

impl Serialize for Accuracy {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Accuracy::Percent(non_zero) => non_zero.serialize(serializer),
            Accuracy::Always => 0u8.serialize(serializer),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveCategory {
    Physical = 0,
    Special = 1,
    Status = 2,
}

impl<'de> Deserialize<'de> for MoveCategory {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match u8::deserialize(deserializer)? {
            0 => Ok(Self::Physical),
            1 => Ok(Self::Special),
            2 => Ok(Self::Status),
            other => Err(serde::de::Error::invalid_value(
                Unexpected::Unsigned(other as u64),
                &"0, 1 or 2",
            )),
        }
    }
}

impl Serialize for MoveCategory {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        (*self as u8).serialize(serializer)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveTarget {
    None,
    User,
    NearAlly,
    UserOrNearAlly,
    UserAndAllies,
    NearFoe,
    RandomNearFoe,
    AllNearFoes,
    Other,
    NearOther,
    AllNearOthers,
    AllBattlers,
    UserSide,
    FoeSide,
    BothSides,
}

pub struct MoveDexDeser<'a>(pub &'a TypeDex);

impl<'a, 'de> DeserializeSeed<'de> for MoveDexDeser<'a> {
    type Value = MoveDex;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct _MoveDetails {
            #[serde(rename = "@real_name")]
            real_name: Box<str>,
            #[serde(rename = "@function_code")]
            function_code: Box<str>,
            #[serde(rename = "@base_damage")]
            base_damage: u8,
            #[serde(rename = "@type")]
            ty: Box<str>,
            #[serde(rename = "@category")]
            category: MoveCategory,
            #[serde(rename = "@accuracy")]
            accuracy: Accuracy,
            #[serde(rename = "@total_pp")]
            total_pp: u8,
            #[serde(rename = "@effect_chance")]
            effect_chance: u8,
            #[serde(rename = "@target")]
            target: MoveTarget,
            #[serde(rename = "@priority")]
            priority: i8,
            #[serde(rename = "@flags")]
            flags: MoveFlags,
            #[serde(rename = "@real_description")]
            description: Box<str>,
        }

        struct _DexVis<'a>(&'a TypeDex);
        impl<'a, 'de> Visitor<'de> for _DexVis<'a> {
            type Value = MoveDex;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                "a ruby hash".fmt(formatter)
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut moves = IndexMap::new();

                while let Some(key) = map.next_key::<MixedKeyRef>()? {
                    match key {
                        MixedKeyRef::Int(_) => {
                            map.next_value::<IgnoredAny>()?;
                        }
                        MixedKeyRef::Str(sym) => {
                            let _MoveDetails {
                                real_name,
                                function_code,
                                base_damage,
                                ty,
                                category,
                                accuracy,
                                total_pp,
                                effect_chance,
                                target,
                                priority,
                                flags,
                                description,
                            } = map.next_value::<_MoveDetails>()?;

                            let ty_id = self.0.get_id_of(&ty).ok_or_else(|| {
                                serde::de::Error::custom(format_args!(
                                    "Type {} not found in TypeDex",
                                    ty
                                ))
                            })?;
                            moves.insert(
                                Box::from(sym),
                                MoveDetails {
                                    name: real_name,
                                    function_code,
                                    power: NonZeroU8::new(base_damage),
                                    ty: ty_id,
                                    category,
                                    accuracy,
                                    pp: total_pp,
                                    effect_chance: NonZeroU8::new(effect_chance),
                                    target,
                                    priority,
                                    flags,
                                    description,
                                },
                            );
                        }
                    }
                }

                moves.shrink_to_fit();
                Ok(MoveDex { map: moves })
            }
        }

        deserializer.deserialize_map(_DexVis(self.0))
    }
}

#[cfg(test)]
pub(crate) mod test {

    use crate::{
        infinite_fusion::{
            Dex,
            moves::{MoveCategory, MoveDex, MoveDexDeser},
        },
        test::infinite_fusion_dir,
    };
    use reikland::DeserializerConfig;
    use serde::de::DeserializeSeed;

    pub(crate) fn load_moves() -> MoveDex {
        let types = crate::infinite_fusion::types::test::load_types();

        let data = std::fs::read(infinite_fusion_dir().join(MoveDex::relative_path())).unwrap();
        let mut deser =
            reikland::Deserializer::with_config(&data, DeserializerConfig::opinionated()).unwrap();

        MoveDexDeser(&types).deserialize(&mut deser).unwrap()
    }

    #[test]
    fn deser_moves_dat() {
        let moves = load_moves();
        assert!(!moves.is_empty());

        let tackle = moves.get_by_key("TACKLE").expect("TACKLE should exist");
        assert_eq!(tackle.category, MoveCategory::Physical);
        assert!(tackle.power.is_some());
    }
}
