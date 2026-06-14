use roaring::RoaringBitmap;

use crate::infinite_fusion::{
    Dex, DexId,
    filters::{HasMove, and_in, separable_filter},
    moves::{MoveDex, MoveId},
    species::{SpeciesDex, SpeciesId},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveSource {
    LevelUp,
    Tutor,
    Egg,
    Any,
}

/// Per-move species sets, split by learn source.
#[derive(Debug, Clone)]
pub struct MoveFilterIndex {
    n_species: usize,
    level_up: Box<[RoaringBitmap]>, // indexed by MoveId
    tutor: Box<[RoaringBitmap]>,
    egg: Box<[RoaringBitmap]>,
}

impl MoveFilterIndex {
    pub fn build(species: &SpeciesDex, moves: &MoveDex) -> Self {
        let mut level_up = vec![RoaringBitmap::new(); moves.len()];
        let mut tutor = vec![RoaringBitmap::new(); moves.len()];
        let mut egg = vec![RoaringBitmap::new(); moves.len()];

        for (id, s) in species.map().values().enumerate() {
            let id = id as u32;
            for level_move in &s.moves {
                level_up[level_move.move_id().to_usize()].insert(id);
            }
            for m in &s.tutor_moves {
                tutor[m.to_usize()].insert(id);
            }
            for m in &s.egg_moves {
                egg[m.to_usize()].insert(id);
            }
        }

        Self {
            n_species: species.len(),
            level_up: level_up.into_boxed_slice(),
            tutor: tutor.into_boxed_slice(),
            egg: egg.into_boxed_slice(),
        }
    }

    pub fn species_with(&self, move_id: MoveId, source: MoveSource) -> RoaringBitmap {
        let i = move_id.to_usize();
        match source {
            MoveSource::LevelUp => self.level_up[i].clone(),
            MoveSource::Tutor => self.tutor[i].clone(),
            MoveSource::Egg => self.egg[i].clone(),
            MoveSource::Any => &(&self.level_up[i] | &self.tutor[i]) | &self.egg[i],
        }
    }

    pub fn filter(&self, move_id: MoveId, source: MoveSource) -> RoaringBitmap {
        separable_filter(self.n_species, &self.species_with(move_id, source))
    }

    /// Species that can learn `move_id` via any of the flagged sources.
    fn species_with_flags(
        &self,
        move_id: MoveId,
        egg: bool,
        level: bool,
        tutor: bool,
    ) -> RoaringBitmap {
        let i = move_id.to_usize();
        let mut set = RoaringBitmap::new();
        if level {
            set |= &self.level_up[i];
        }
        if tutor {
            set |= &self.tutor[i];
        }
        if egg {
            set |= &self.egg[i];
        }
        set
    }

    /// Per-head body set: bodies covering *every* requested move the head can't itself learn where `None` = the head already learns them all).
    /// A conjunction across `has_move.moves`.
    pub fn bodies_for_head(&self, head: SpeciesId, has_move: &HasMove) -> Option<RoaringBitmap> {
        let head = head.to_u32();
        let mut acc: Option<RoaringBitmap> = None;
        for &move_id in &has_move.moves {
            let learners =
                self.species_with_flags(move_id, has_move.egg, has_move.level, has_move.tutor);
            // if the head can't learn this move, the body must otherwise no constraint from it
            if !learners.contains(head) {
                and_in(&mut acc, learners);
            }
        }
        acc
    }
}

#[cfg(test)]
mod test {
    use roaring::RoaringBitmap;

    use crate::{
        infinite_fusion::{
            Dex, DexId, GameVersion, InfiniteFusionDex,
            filters::move_filter::{MoveFilterIndex, MoveSource},
            species::SpeciesId,
        },
        test::infinite_fusion_dir,
    };

    #[test]
    fn move_filter_matches_a_brute_force_scan() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let species = dex.species();
        let n = species.len();
        let index = MoveFilterIndex::build(species, dex.moves());

        let tackle = dex.moves().get_id_of("TACKLE").expect("TACKLE exists");

        let base: Vec<_> = (0..n)
            .map(|i| species.get_item(SpeciesId::from_usize(i)))
            .collect();
        let learns = |i: usize| {
            base[i].moves.iter().any(|m| m.move_id() == tackle)
                || base[i].tutor_moves.contains(&tackle)
                || base[i].egg_moves.contains(&tackle)
        };
        let oracle: RoaringBitmap = (0..n)
            .flat_map(|h| (0..n).map(move |b| (h, b)))
            .filter(|&(h, b)| learns(h) || learns(b))
            .map(|(h, b)| (h * n + b) as u32)
            .collect();

        let result = index.filter(tackle, MoveSource::Any);
        assert!(!result.is_empty());
        assert_eq!(result, oracle);
    }
}
