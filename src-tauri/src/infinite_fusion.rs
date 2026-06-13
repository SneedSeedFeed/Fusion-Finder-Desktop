use std::{ops::Deref, path::Path};

use indexmap::IndexMap;
use serde::{Deserialize, de::DeserializeSeed};

pub mod abilities;
pub mod items;
pub mod moves;
pub mod species;
pub mod types;

pub trait DexId {
    fn from_usize(v: usize) -> Self;
    fn to_usize(self) -> usize;
}

/// Immutable store of data for Pokemon Infinite Fusion. Any instance of [`Self::Id`] SHOULD always be valid.
pub trait Dex {
    /// Relative path from the root of InfiniteFusion to find the relevant file
    fn relative_path() -> &'static Path;

    type Id: DexId;
    type Item;

    fn map(&self) -> &IndexMap<Box<str>, Self::Item>;

    fn get(&self, id: Self::Id) -> (&str, &Self::Item) {
        self.map()
            .get_index(id.to_usize())
            .map(|(k, v)| (k.deref(), v))
            .expect("unmapped id")
    }

    fn len(&self) -> usize {
        self.map().len()
    }

    fn is_empty(&self) -> bool {
        self.map().is_empty()
    }

    fn get_opt(&self, index: usize) -> Option<(&str, &Self::Item)> {
        self.map().get_index(index).map(|(k, v)| (k.deref(), v))
    }

    fn get_by_key(&self, key: &str) -> Option<&Self::Item> {
        self.map().get(key)
    }

    fn get_id_of(&self, key: &str) -> Option<Self::Id> {
        self.map()
            .get_full(key)
            .map(|(i, _, _)| Self::Id::from_usize(i))
    }

    fn get_full_by_key(&self, key: &str) -> Option<(Self::Id, &Self::Item)> {
        self.map()
            .get_full(key)
            .map(|(i, _, v)| (Self::Id::from_usize(i), v))
    }
}

impl<T> Dex for &T
where
    T: Dex,
{
    fn relative_path() -> &'static Path {
        T::relative_path()
    }

    type Id = T::Id;

    type Item = T::Item;

    fn map(&self) -> &IndexMap<Box<str>, Self::Item> {
        todo!()
    }

    fn get(&self, id: Self::Id) -> (&str, &Self::Item) {
        <T as Dex>::get(self, id)
    }

    fn len(&self) -> usize {
        <T as Dex>::len(self)
    }

    fn is_empty(&self) -> bool {
        <T as Dex>::is_empty(self)
    }

    fn get_opt(&self, index: usize) -> Option<(&str, &Self::Item)> {
        <T as Dex>::get_opt(self, index)
    }

    fn get_by_key(&self, key: &str) -> Option<&Self::Item> {
        <T as Dex>::get_by_key(self, key)
    }

    fn get_id_of(&self, key: &str) -> Option<Self::Id> {
        <T as Dex>::get_id_of(self, key)
    }

    fn get_full_by_key(&self, key: &str) -> Option<(Self::Id, &Self::Item)> {
        <T as Dex>::get_full_by_key(self, key)
    }
}

pub struct DexIdKeyVisitor<'a, T>(pub &'a T);

impl<'de, 'a, T> DeserializeSeed<'de> for DexIdKeyVisitor<'a, T>
where
    T: Dex,
{
    type Value = T::Id;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        <&str as Deserialize>::deserialize(deserializer).and_then(|key| {
            self.0
                .get_id_of(key)
                .ok_or_else(|| serde::de::Error::custom(format_args!("{key} not found in dex")))
        })
    }
}
