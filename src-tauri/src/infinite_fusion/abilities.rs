use std::{fmt::Display, path::Path};

use indexmap::IndexMap;
use reikland::MixedKeyRef;
use serde::{
    Deserialize, Serialize,
    de::{IgnoredAny, Visitor},
};

use crate::{dex_id, infinite_fusion::Dex};

pub struct AbilityDex {
    map: IndexMap<Box<str>, AbilityDetails>,
}

impl Dex for AbilityDex {
    fn relative_path() -> &'static Path {
        Path::new("Data/abilities.dat")
    }

    type Id = AbilityId;

    type Item = AbilityDetails;

    fn map(&self) -> &IndexMap<Box<str>, Self::Item> {
        &self.map
    }
}

dex_id!(AbilityId, u16);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AbilityDetails {
    pub name: Box<str>,
    pub description: Box<str>,
}

impl<'de> Deserialize<'de> for AbilityDex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(AbilityDexVisitor)
    }
}

struct AbilityDexVisitor;

impl<'de> Visitor<'de> for AbilityDexVisitor {
    type Value = AbilityDex;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        "a ruby hash".fmt(formatter)
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        #[derive(Deserialize)]
        struct _AbilityDetails {
            #[serde(rename = "@real_name")]
            real_name: Box<str>,
            #[serde(rename = "@real_description")]
            real_description: Box<str>,
        }

        let mut abilities = IndexMap::new();

        while let Some(key) = map.next_key::<MixedKeyRef>()? {
            match key {
                MixedKeyRef::Int(_) => {
                    map.next_value::<IgnoredAny>()?;
                }
                MixedKeyRef::Str(sym) => {
                    let _AbilityDetails {
                        real_name,
                        real_description,
                    } = map.next_value::<_AbilityDetails>()?;

                    abilities.insert(
                        Box::from(sym),
                        AbilityDetails {
                            name: real_name,
                            description: real_description,
                        },
                    );
                }
            }
        }

        abilities.shrink_to_fit();
        Ok(AbilityDex { map: abilities })
    }
}

#[cfg(test)]
pub(crate) mod test {
    use reikland::DeserializerConfig;

    use crate::{
        infinite_fusion::{Dex, abilities::AbilityDex},
        test::infinite_fusion_dir,
    };

    pub(crate) fn load_abilities() -> AbilityDex {
        let data = std::fs::read(infinite_fusion_dir().join(AbilityDex::relative_path())).unwrap();

        reikland::from_bytes_with_config::<AbilityDex>(&data, DeserializerConfig::opinionated())
            .unwrap()
    }

    #[test]
    fn deser_abilities_dat() {
        let abilities = load_abilities();
        assert!(!abilities.is_empty());

        let stench = abilities.get_by_key("STENCH").expect("STENCH should exist");
        assert!(!stench.name.is_empty());
        assert!(!stench.description.is_empty());
    }
}
