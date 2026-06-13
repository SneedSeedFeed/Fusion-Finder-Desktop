use std::{fmt::Display, path::Path};

use indexmap::IndexMap;
use reikland::MixedKeyRef;
use serde::{
    Deserialize, Serialize,
    de::{DeserializeSeed, IgnoredAny, Visitor},
};

use crate::{
    dex_id,
    infinite_fusion::{
        Dex,
        moves::{MoveDex, MoveId},
    },
};

#[derive(Debug, Clone)]
pub struct ItemDex {
    map: IndexMap<Box<str>, ItemDetails>,
}

impl Dex for ItemDex {
    fn relative_path() -> &'static Path {
        Path::new("Data/items.dat")
    }

    type Id = ItemId;

    type Item = ItemDetails;

    fn map(&self) -> &IndexMap<Box<str>, Self::Item> {
        &self.map
    }
}

dex_id!(ItemId, u16);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ItemDetails {
    pub name: Box<str>,
    pub description: Box<str>,
    pub move_taught: Option<MoveId>,
}
pub struct ItemDexDeser<'a>(pub &'a MoveDex);

impl<'a, 'de> DeserializeSeed<'de> for ItemDexDeser<'a> {
    type Value = ItemDex;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct _ItemDetails {
            #[serde(rename = "@real_name")]
            real_name: Box<str>,
            #[serde(rename = "@real_description")]
            real_description: Box<str>,
            #[serde(rename = "@move")]
            move_taught: Option<Box<str>>,
        }

        struct _DexVis<'a>(&'a MoveDex);
        impl<'a, 'de> Visitor<'de> for _DexVis<'a> {
            type Value = ItemDex;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                "a ruby hash".fmt(formatter)
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut items = IndexMap::new();

                while let Some(key) = map.next_key::<MixedKeyRef>()? {
                    match key {
                        MixedKeyRef::Int(_) => {
                            map.next_value::<IgnoredAny>()?;
                        }
                        MixedKeyRef::Str(sym) => {
                            let _ItemDetails {
                                real_name,
                                real_description,
                                move_taught,
                            } = map.next_value::<_ItemDetails>()?;

                            let move_taught = move_taught
                                .map(|sym| {
                                    self.0.get_id_of(&sym).ok_or_else(|| {
                                        serde::de::Error::custom(format_args!(
                                            "Move {sym} taught by item {} not found in MoveDex",
                                            real_name
                                        ))
                                    })
                                })
                                .transpose()?;

                            items.insert(
                                Box::from(sym),
                                ItemDetails {
                                    name: real_name,
                                    description: real_description,
                                    move_taught,
                                },
                            );
                        }
                    }
                }

                items.shrink_to_fit();
                Ok(ItemDex { map: items })
            }
        }

        deserializer.deserialize_map(_DexVis(self.0))
    }
}

#[cfg(test)]
pub(crate) mod test {
    use reikland::DeserializerConfig;
    use serde::de::DeserializeSeed;

    use crate::{
        infinite_fusion::{Dex, items::ItemDex, items::ItemDexDeser},
        test::infinite_fusion_dir,
    };

    pub(crate) fn load_items() -> ItemDex {
        let moves = crate::infinite_fusion::moves::test::load_moves();

        let data = std::fs::read(infinite_fusion_dir().join(ItemDex::relative_path())).unwrap();
        let mut deser =
            reikland::Deserializer::with_config(&data, DeserializerConfig::opinionated()).unwrap();

        ItemDexDeser(&moves).deserialize(&mut deser).unwrap()
    }

    #[test]
    fn deser_items_dat() {
        let items = load_items();
        assert!(!items.is_empty());

        let repel = items.get_by_key("REPEL").expect("REPEL should exist");
        assert!(repel.move_taught.is_none());

        let tm01 = items.get_by_key("TM01").expect("TM01 should exist");
        assert!(tm01.move_taught.is_some());
    }
}
