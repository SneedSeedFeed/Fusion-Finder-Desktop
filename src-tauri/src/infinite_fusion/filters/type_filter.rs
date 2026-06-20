use roaring::RoaringBitmap;

use crate::infinite_fusion::{
    Dex, DexId,
    species::{SpeciesDetails, SpeciesDex, SpeciesId},
    types::{TypeDex, TypeId},
};

fn head_type_of(s: &SpeciesDetails, types: &TypeDex) -> TypeId {
    match s.type2 {
        Some(type2) if types.is_normal(s.type1) && types.is_flying(type2) => type2,
        _ => s.type1,
    }
}

fn body_type_of(s: &SpeciesDetails, fusion_type1: TypeId) -> TypeId {
    let raw_secondary = s.type2.unwrap_or(s.type1);
    if raw_secondary == fusion_type1 {
        s.type1
    } else {
        raw_secondary
    }
}

/// The two types of the fusion `head`/`body`
pub fn fused_types(
    head: &SpeciesDetails,
    body: &SpeciesDetails,
    types: &TypeDex,
) -> (TypeId, Option<TypeId>) {
    let type1 = head_type_of(head, types);
    let type2 = body_type_of(body, type1);
    (type1, (type2 != type1).then_some(type2))
}

/// For type filtering. `head_type` is purely per-species, but the body's contribution depends on
/// the head's type (the duplicate-avoidance rule), so `body_type` is indexed by *both*.
#[derive(Debug, Clone)]
pub struct TypeFilterIndex {
    #[cfg(test)]
    n_species: usize,
    /// head-contributed type per species
    head_type: Vec<TypeId>,
    /// `[head_type][contributed]` -> bodies whose contribution is `contributed` under that head_type
    body_type: Vec<Vec<RoaringBitmap>>,
}

impl TypeFilterIndex {
    pub fn build(species: &SpeciesDex, types: &TypeDex) -> Self {
        let n_types = types.len();

        let head_type: Vec<TypeId> = species
            .map()
            .values()
            .map(|s| head_type_of(s, types))
            .collect();

        let mut body_type: Vec<Vec<RoaringBitmap>> = (0..n_types)
            .map(|_| vec![RoaringBitmap::new(); n_types])
            .collect();
        for (id, s) in species.map().values().enumerate() {
            let id = id as u32;
            for (t1, row) in body_type.iter_mut().enumerate() {
                let contributed = body_type_of(s, TypeId::from_usize(t1));
                row[contributed.to_usize()].insert(id);
            }
        }

        Self {
            #[cfg(test)]
            n_species: species.len(),
            head_type,
            body_type,
        }
    }

    /// Per-head body set for the type filter (`None` = no type requested). `types` is 1 or 2 entries.
    pub fn bodies_for_head(&self, head: SpeciesId, types: &[TypeId]) -> Option<RoaringBitmap> {
        let head_type = self.head_type[head.to_usize()];
        match types {
            [] => None,
            [ty] => {
                if head_type == *ty {
                    None // head already supplies the type every body qualifies
                } else {
                    Some(self.body_type[head_type.to_usize()][ty.to_usize()].clone())
                }
            }
            // dual: the head must supply one type and the body the other
            [a, b, ..] => {
                let needed = if head_type == *a {
                    *b
                } else if head_type == *b {
                    *a
                } else {
                    return Some(RoaringBitmap::new()); // head supplies neither no body works
                };
                Some(self.body_type[head_type.to_usize()][needed.to_usize()].clone())
            }
        }
    }
}

#[cfg(test)]
mod test {
    use roaring::RoaringBitmap;

    use crate::{
        infinite_fusion::{
            Dex, DexId, GameVersion, InfiniteFusionDex,
            filters::type_filter::{TypeFilterIndex, body_type_of, head_type_of},
            species::SpeciesId,
            types::TypeId,
        },
        test::infinite_fusion_dir,
    };

    impl TypeFilterIndex {
        /// Every fusion that has type `ty`.
        pub fn filter(&self, ty: TypeId) -> RoaringBitmap {
            let n = self.n_species;
            let mut result = RoaringBitmap::new();
            for head in 0..n {
                let row = (head * n) as u32;
                if self.head_type[head] == ty {
                    // head supplies `ty`, so every body yields a fusion that has it
                    result.insert_range(row..row + n as u32);
                } else {
                    let bodies = &self.body_type[self.head_type[head].to_usize()][ty.to_usize()];
                    result.extend(bodies.iter().map(|body| row + body));
                }
            }
            result
        }

        /// Every fusion that has *both* `a` and `b`. Pass the same type twice to require mono-`a`.
        pub fn filter_dual(&self, a: TypeId, b: TypeId) -> RoaringBitmap {
            let n = self.n_species;
            let mut result = RoaringBitmap::new();
            for head in 0..n {
                let head_type = self.head_type[head];
                // a fusion's two types are {head_type, body_type}; to hold both `a` and `b` the head
                // must supply one and the body the other.
                let needed = if head_type == a {
                    b
                } else if head_type == b {
                    a
                } else {
                    continue;
                };
                let row = (head * n) as u32;
                let bodies = &self.body_type[head_type.to_usize()][needed.to_usize()];
                result.extend(bodies.iter().map(|body| row + body));
            }
            result
        }
    }

    #[test]
    fn type_filter_matches_a_brute_force_scan() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let species = dex.species();
        let types = dex.types();
        let n = species.len();
        let index = TypeFilterIndex::build(species, types);

        let water = types.get_id_of("WATER").expect("WATER exists");

        let base: Vec<_> = (0..n)
            .map(|i| species.get_item(SpeciesId::from_usize(i)))
            .collect();
        // a fusion has a type if either contributed type equals it (using the game's rules)
        let has_water = |h: usize, b: usize| {
            let t1 = head_type_of(base[h], types);
            t1 == water || body_type_of(base[b], t1) == water
        };
        let oracle: RoaringBitmap = (0..n)
            .flat_map(|h| (0..n).map(move |b| (h, b)))
            .filter(|&(h, b)| has_water(h, b))
            .map(|(h, b)| (h * n + b) as u32)
            .collect();

        let single = index.filter(water);
        assert!(!single.is_empty());
        assert_eq!(single, oracle);

        // a dual filter is a subset of each single filter it contains
        let grass = types.get_id_of("GRASS").expect("GRASS exists");
        let dual = index.filter_dual(water, grass);
        assert!(dual.is_subset(&single));
        assert!(dual.is_subset(&index.filter(grass)));
    }
}
