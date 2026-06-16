//! For a given location, what pokemon are there?

use std::{cmp::Reverse, sync::Arc};

use serde::Serialize;

use crate::infinite_fusion::{
    Dex, InfiniteFusionDex,
    encounters::{EncounterMethod, EncounterMode},
    species::SpeciesId,
    types::TypeId,
};

#[derive(Debug, Clone, Serialize)]
pub struct AreaEncounter {
    pub species: SpeciesId,
    pub dex_id: u16,
    pub name: Box<str>,
    pub types: (TypeId, Option<TypeId>),
    pub method: EncounterMethod,
    pub chance: u8,
    pub min_level: u8,
    pub max_level: u8,
    pub mode: EncounterMode,
}

impl InfiniteFusionDex {
    /// Every distinct encounter location, deduped and sorted, for the area picker.
    pub fn locations(&self) -> Box<[Arc<str>]> {
        let mut locations: Vec<Arc<str>> = self
            .encounters()
            .all()
            .iter()
            .map(|e| e.route.clone())
            .collect();
        locations.sort();
        locations.dedup();
        locations.into_boxed_slice()
    }

    /// Every encounter at `location` (all modes), clustered by method and sorted by chance
    pub fn area_encounters(&self, location: &str) -> Box<[AreaEncounter]> {
        let mut rows: Vec<AreaEncounter> = self
            .encounters()
            .all()
            .iter()
            .filter(|e| &*e.route == location)
            .map(|e| {
                let s = self.species().get_item(e.species);
                AreaEncounter {
                    species: e.species,
                    dex_id: s.id_number,
                    name: s.name.clone(),
                    types: (s.type1, s.type2),
                    method: e.method,
                    chance: e.chance,
                    min_level: e.min_level,
                    max_level: e.max_level,
                    mode: e.mode,
                }
            })
            .collect();
        rows.sort_by_key(|a| (a.method as u8, Reverse(a.chance)));
        rows.into_boxed_slice()
    }
}

#[cfg(test)]
mod test {
    use crate::{
        infinite_fusion::{GameVersion, InfiniteFusionDex, encounters::EncounterMethod},
        test::infinite_fusion_dir,
    };

    #[test]
    fn route_24_lists_abra() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();

        let locations = dex.locations();
        assert!(!locations.is_empty());
        assert!(locations.iter().any(|l| &**l == "Route 24"));

        // Abra is a Route 24 grass encounter
        let route24 = dex.area_encounters("Route 24");
        assert!(
            route24
                .iter()
                .any(|e| &*e.name == "Abra" && e.method == EncounterMethod::Land)
        );

        // sort key holds: methods cluster, highest chance first within a method.
        for w in route24.windows(2) {
            let key = |e: &super::AreaEncounter| (e.method as u8, std::cmp::Reverse(e.chance));
            assert!(key(&w[0]) <= key(&w[1]));
        }
    }
}
