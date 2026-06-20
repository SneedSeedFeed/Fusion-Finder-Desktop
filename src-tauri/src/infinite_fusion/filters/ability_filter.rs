use roaring::RoaringBitmap;

use crate::infinite_fusion::{
    Dex, DexId,
    abilities::{AbilityDex, AbilityId},
    filters::HasAbility,
    species::{SpeciesDex, SpeciesId},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilitySource {
    Regular,
    Hidden,
    Any,
}

/// Per-ability species sets, split by slot (regular vs hidden).
#[derive(Debug, Clone)]
pub struct AbilityFilterIndex {
    #[cfg(test)]
    n_species: usize,
    regular: Box<[RoaringBitmap]>, // indexed by AbilityId
    hidden: Box<[RoaringBitmap]>,
}

impl AbilityFilterIndex {
    pub fn build(species: &SpeciesDex, abilities: &AbilityDex) -> Self {
        let mut regular = vec![RoaringBitmap::new(); abilities.len()];
        let mut hidden = vec![RoaringBitmap::new(); abilities.len()];

        for (id, s) in species.map().values().enumerate() {
            let id = id as u32;
            for ability in &s.abilities {
                regular[ability.to_usize()].insert(id);
            }
            for ability in &s.hidden_abilities {
                hidden[ability.to_usize()].insert(id);
            }
        }

        Self {
            #[cfg(test)]
            n_species: species.len(),
            regular: regular.into_boxed_slice(),
            hidden: hidden.into_boxed_slice(),
        }
    }

    pub fn species_with(&self, ability: AbilityId, source: AbilitySource) -> RoaringBitmap {
        let i = ability.to_usize();
        match source {
            AbilitySource::Regular => self.regular[i].clone(),
            AbilitySource::Hidden => self.hidden[i].clone(),
            AbilitySource::Any => &self.regular[i] | &self.hidden[i],
        }
    }

    /// Per-head body set (`None` = the head already has the ability, so every body qualifies).
    pub fn bodies_for_head(&self, head: SpeciesId, ability: &HasAbility) -> Option<RoaringBitmap> {
        let (id, source) = match ability {
            HasAbility::Normal(a) => (*a, AbilitySource::Regular),
            HasAbility::Hidden(a) => (*a, AbilitySource::Hidden),
            HasAbility::Either(a) => (*a, AbilitySource::Any),
        };
        let species = self.species_with(id, source);
        if species.contains(head.to_u32()) {
            None
        } else {
            Some(species)
        }
    }
}

#[cfg(test)]
mod test {
    use roaring::RoaringBitmap;

    use crate::{
        infinite_fusion::{
            Dex, DexId, GameVersion, InfiniteFusionDex,
            abilities::AbilityId,
            filters::{
                ability_filter::{AbilityFilterIndex, AbilitySource},
                test::separable_filter,
            },
            species::SpeciesId,
        },
        test::infinite_fusion_dir,
    };

    impl AbilityFilterIndex {
        pub fn filter(&self, ability: AbilityId, source: AbilitySource) -> RoaringBitmap {
            separable_filter(self.n_species, &self.species_with(ability, source))
        }
    }

    #[test]
    fn ability_filter_matches_a_brute_force_scan() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let species = dex.species();
        let n = species.len();
        let index = AbilityFilterIndex::build(species, dex.abilities());

        let overgrow = dex
            .abilities()
            .get_id_of("OVERGROW")
            .expect("OVERGROW exists");

        let base: Vec<_> = (0..n)
            .map(|i| species.get_item(SpeciesId::from_usize(i)))
            .collect();
        // "any slot" oracle. head or body lists overgrow in either ability list
        let has_overgrow = |i: usize| {
            base[i].abilities.contains(&overgrow) || base[i].hidden_abilities.contains(&overgrow)
        };
        let oracle: RoaringBitmap = (0..n)
            .flat_map(|h| (0..n).map(move |b| (h, b)))
            .filter(|&(h, b)| has_overgrow(h) || has_overgrow(b))
            .map(|(h, b)| (h * n + b) as u32)
            .collect();

        let result = index.filter(overgrow, AbilitySource::Any);
        assert!(!result.is_empty());
        assert_eq!(result, oracle);

        // a hidden-only query is a subset of the any-slot one
        let hidden = index.filter(overgrow, AbilitySource::Hidden);
        assert!(hidden.is_subset(&result));
    }
}
