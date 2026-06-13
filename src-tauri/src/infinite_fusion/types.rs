use std::{
    fmt::Display,
    iter::{Filter, Map},
    ops::{Deref, Range},
    path::Path,
};

use indexmap::{IndexMap, IndexSet};
use reikland::MixedKeyRef;
use serde::{Deserialize, Serialize, de::Visitor, ser::SerializeSeq};

use crate::{dex_id, infinite_fusion::Dex};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TypeSet(u32);

impl TypeSet {
    pub fn new() -> Self {
        Self(0)
    }

    pub fn insert(&mut self, ty: TypeId) {
        self.0 |= 1u32 << ty.0;
    }

    pub fn contains(self, ty: TypeId) -> bool {
        self.0 & (1u32 << ty.0) != 0
    }

    #[allow(clippy::type_complexity)]
    pub fn iter(self) -> Map<Filter<Range<u8>, impl FnMut(&u8) -> bool>, fn(u8) -> TypeId> {
        (0..u32::BITS as u8)
            .filter(move |i| self.0 & (1u32 << i) != 0)
            .map(TypeId)
    }
}

impl<'de> Deserialize<'de> for TypeSet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Vis;

        impl<'de> Visitor<'de> for Vis {
            type Value = TypeSet;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                "a sequence of positive 8 bit integers".fmt(formatter)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut set = TypeSet::default();
                while let Some(elem) = seq.next_element::<TypeId>()? {
                    set.insert(elem);
                }
                Ok(set)
            }
        }
        deserializer.deserialize_seq(Vis)
    }
}

impl Serialize for TypeSet {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut seq = serializer.serialize_seq(None)?;
        self.iter().try_for_each(|id| seq.serialize_element(&id))?;
        seq.end()
    }
}

impl FromIterator<TypeId> for TypeSet {
    fn from_iter<T: IntoIterator<Item = TypeId>>(iter: T) -> Self {
        iter.into_iter().fold(TypeSet::default(), |mut acc, elem| {
            acc.insert(elem);
            acc
        })
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TypeDetails {
    pub name: Box<str>,
    pub weaknesses: TypeSet,
    pub resistances: TypeSet,
    pub immunities: TypeSet,
}

dex_id!(TypeId, u8);

pub struct TypeDex {
    // id of normal and flying types are tracked explicitly for use in fusion type checking
    flying_id: TypeId,
    normal_id: TypeId,
    map: IndexMap<Box<str>, TypeDetails>,
}

impl TypeDex {
    pub fn is_normal_flying(&self, primary: TypeId, secondary: TypeId) -> bool {
        primary == self.normal_id && secondary == self.flying_id
    }

    pub fn is_normal(&self, ty: TypeId) -> bool {
        self.normal_id == ty
    }

    pub fn is_flying(&self, ty: TypeId) -> bool {
        self.flying_id == ty
    }
}

impl Dex for TypeDex {
    fn relative_path() -> &'static std::path::Path {
        Path::new("Data/types.dat")
    }

    type Id = TypeId;

    type Item = TypeDetails;

    fn map(&self) -> &IndexMap<Box<str>, Self::Item> {
        &self.map
    }
}

impl<'de> Deserialize<'de> for TypeDex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_map(TypeDexVisitor)
    }
}

struct TypeDexVisitor;

impl<'de> Visitor<'de> for TypeDexVisitor {
    type Value = TypeDex;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("a ruby hash")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut indeces = IndexSet::with_capacity(18);
        let mut types = IndexMap::with_capacity(18);

        let mut flying_idx = None::<u8>;
        let mut normal_idx = None::<u8>;

        #[derive(Deserialize)]
        struct _TypeDetails {
            // as_transparent since iirc the string is ivar wrapped
            #[serde(rename = "@real_name")]
            real_name: Box<str>,
            #[serde(rename = "@weaknesses")]
            weaknesses: Box<[Box<str>]>,
            #[serde(rename = "@resistances")]
            resistances: Box<[Box<str>]>,
            #[serde(rename = "@immunities")]
            immunities: Box<[Box<str>]>,
        }

        while let Some(key) = map.next_key::<MixedKeyRef>()? {
            match key {
                MixedKeyRef::Int(_) => map.next_value::<serde::de::IgnoredAny>().map(|_| {})?,
                MixedKeyRef::Str(sym) => {
                    let idx = indeces.len() as u8;

                    if idx >= u32::BITS as u8 {
                        return Err(serde::de::Error::custom(
                            "TypeSet can only represent up to 32 types, go yell at the dev to increase the limit to 64",
                        ));
                    }

                    match sym {
                        "FLYING" => flying_idx = Some(idx),
                        "NORMAL" => normal_idx = Some(idx),
                        _ => {}
                    }

                    let val = map.next_value::<_TypeDetails>()?;

                    if val.real_name.contains('/') {
                        // skips triple types since I have no plans of implementing them yet
                        continue;
                    }

                    indeces.insert(sym);

                    types.insert(Box::<str>::from(sym), val);
                }
            }
        }

        let mut map = IndexMap::with_capacity(types.len());

        #[allow(clippy::borrowed_box)] // .iter() is Item=&Box<str> here
        let grab_id = |str: &Box<str>| {
            indeces
                .get_full(str.deref())
                .map(|(i, _)| TypeId(i as u8))
                .ok_or_else(|| serde::de::Error::custom(format_args!("Type Id '{str}' not found")))
        };

        for (k, v) in types {
            let resistances = v
                .resistances
                .iter()
                .map(grab_id)
                .collect::<Result<_, _>>()?;

            let weaknesses = v.weaknesses.iter().map(grab_id).collect::<Result<_, _>>()?;

            let immunities = v.immunities.iter().map(grab_id).collect::<Result<_, _>>()?;

            map.insert(
                k,
                TypeDetails {
                    name: v.real_name,
                    weaknesses,
                    resistances,
                    immunities,
                },
            );
        }

        map.shrink_to_fit(); // immutable so might as well reclaim
        Ok(TypeDex {
            flying_id: flying_idx
                .ok_or_else(|| serde::de::Error::custom("type dex did not contain the flying type, its presence is essential for NORMAL/FLYING fusion logic"))
                .map(TypeId)?,
            normal_id: normal_idx
                .ok_or_else(|| serde::de::Error::custom("type dex did not contain the normal type, its presence is essential for NORMAL/FLYING fusion logic"))
                .map(TypeId)?,
            map,
        })
    }
}

#[cfg(test)]
pub(crate) mod test {

    use reikland::DeserializerConfig;

    use crate::{
        infinite_fusion::{Dex, types::TypeDex},
        test::infinite_fusion_dir,
    };

    pub(crate) fn load_types() -> TypeDex {
        let data = std::fs::read(infinite_fusion_dir().join(TypeDex::relative_path())).unwrap();

        reikland::from_bytes_with_config::<TypeDex>(&data, DeserializerConfig::opinionated())
            .unwrap()
    }

    #[test]
    fn deser_types_dat() {
        let types = load_types();

        // find steel type (my fave)
        let (steel_id, steel_details) = types.get_full_by_key("STEEL").unwrap();
        // find fire
        let (fire_id, fire_details) = types.get_full_by_key("FIRE").unwrap();

        assert!(steel_details.weaknesses.contains(fire_id));
        assert!(fire_details.resistances.contains(steel_id));

        let normal_id = types.normal_id;
        // normal is normal?!?!?!
        let (normal_key, normal_details) = types.get(normal_id);
        let (ghost_id, ghost_details) = types.get_full_by_key("GHOST").unwrap();
        assert_eq!(normal_key, "NORMAL");
        assert!(normal_details.immunities.contains(ghost_id));
        assert!(ghost_details.immunities.contains(normal_id));
    }
}
