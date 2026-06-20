//! inspect view but for moves

use std::{num::NonZeroU8, sync::Arc};

use serde::Serialize;

use crate::infinite_fusion::{
    Dex, InfiniteFusionDex,
    items::ItemId,
    moves::{Accuracy, MoveCategory, MoveId, flags::MoveFlags},
    types::TypeId,
};

/// The machine that teaches a move, plus where to find it
#[derive(Debug, Clone, Serialize)]
pub struct MachineSource {
    pub name: Box<str>,
    pub is_hm: bool,
    /// routes the machine is found on (scraped from map scripts). empty = mart/event-only.
    pub locations: Box<[Arc<str>]>,
}

/// A Move Expert signature move: which expert teaches it, where they are, and who qualifies
#[derive(Debug, Clone, Serialize)]
pub struct ExpertSource {
    pub legendary: bool,
    pub locations: Box<[Arc<str>]>,
    /// plain-English eligibility
    pub condition: Box<str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MoveCard {
    pub name: Box<str>,
    pub ty: TypeId,
    pub category: MoveCategory,
    pub power: Option<NonZeroU8>,
    pub accuracy: Accuracy,
    pub pp: u8,
    pub priority: i8,
    pub effect_chance: Option<NonZeroU8>,
    pub description: Box<str>,
    pub flags: MoveFlags,
    pub machine: Option<MachineSource>,
    pub tutor_locations: Box<[Arc<str>]>,
    /// present iff this is a Move Expert signature move
    pub expert: Option<ExpertSource>,
}

impl InfiniteFusionDex {
    /// The routes where machine `item` is found in the world (scraped from map scripts). 
    /// Empty for machines that are only bought, awarded by events we don't scrape, or otherwise unplaced.
    pub fn tm_locations(&self, item: ItemId) -> Box<[Arc<str>]> {
        self.tm_locations
            .get(&item)
            .cloned()
            .unwrap_or_else(|| Box::new([]))
    }

    /// The hover-card for `move_id`
    pub fn move_card(&self, move_id: MoveId) -> MoveCard {
        let m = self.moves().get_item(move_id);
        let machine = self.machine_moves.get(&move_id).map(|&item| {
            let name = self.items().get_item(item).name.clone();
            MachineSource {
                is_hm: name.starts_with("HM"),
                name,
                locations: self.tm_locations(item),
            }
        });
        MoveCard {
            name: m.name.clone(),
            ty: m.ty,
            category: m.category,
            power: m.power,
            accuracy: m.accuracy,
            pp: m.pp,
            priority: m.priority,
            effect_chance: m.effect_chance,
            description: m.description.clone(),
            flags: m.flags,
            machine,
            tutor_locations: self
                .tutor_locations
                .get(&move_id)
                .cloned()
                .unwrap_or_else(|| Box::new([])),
            expert: self.expert_source(move_id),
        }
    }

    /// The Move Expert source for `move_id`, if it's a signature move
    fn expert_source(&self, move_id: MoveId) -> Option<ExpertSource> {
        let mut rules = self
            .expert_moves
            .iter()
            .filter(|em| em.move_id == move_id)
            .peekable();
        let legendary = rules.peek()?.legendary;
        let locations = if legendary {
            &self.expert_locations.legendary
        } else {
            &self.expert_locations.normal
        };
        let condition = rules
            .map(|r| r.cond.describe(self))
            .collect::<Vec<_>>()
            .join(" or ");
        Some(ExpertSource {
            legendary,
            locations: locations.clone(),
            condition: condition.into(),
        })
    }
}

#[cfg(test)]
mod test {
    use crate::{
        infinite_fusion::{Dex, DexId, GameVersion, InfiniteFusionDex, moves::MoveId},
        test::infinite_fusion_dir,
    };

    #[test]
    fn move_card_carries_machine_and_locations() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let moves = dex.moves();

        // A TM move reports its machine; at least one machine should have a scraped location.
        let mut machine_moves = 0;
        let mut located = 0;
        for i in 0..moves.len() {
            let card = dex.move_card(MoveId::from_usize(i));
            if let Some(machine) = &card.machine {
                machine_moves += 1;
                assert!(machine.name.starts_with("TM") || machine.name.starts_with("HM"));
                if !machine.locations.is_empty() {
                    located += 1;
                }
            }
        }
        assert!(
            machine_moves > 0,
            "expected some moves to be taught by a TM/HM"
        );
        assert!(
            located > 0,
            "expected some machines to have a scraped location"
        );

        // Tackle is a level-up move taught by no machine.
        let tackle = moves.get_id_of("TACKLE").unwrap();
        assert!(dex.move_card(tackle).machine.is_none());
    }

    #[test]
    fn move_card_locates_move_tutors() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let moves = dex.moves();
        let tutor_at = |sym: &str| dex.move_card(moves.get_id_of(sym).unwrap()).tutor_locations;

        // The Pledge tutor is in Cerulean City.
        for pledge in ["FIREPLEDGE", "GRASSPLEDGE", "WATERPLEDGE"] {
            let locs = tutor_at(pledge);
            assert!(
                locs.iter().any(|l| l.contains("Cerulean")),
                "{pledge} tutor should be in Cerulean City, got {locs:?}"
            );
        }

        // Drill Run is tutored on Route 10.
        let drill_run = tutor_at("DRILLRUN");
        assert!(
            drill_run.iter().any(|l| &**l == "Route 10"),
            "Drill Run tutor should be on Route 10, got {drill_run:?}"
        );
    }

    #[test]
    fn move_card_carries_expert_source() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let moves = dex.moves();

        // Attack Order: regular expert (Knot Island), condition names Beedrill.
        let attack_order = dex.move_card(moves.get_id_of("ATTACKORDER").unwrap());
        let expert = attack_order.expert.expect("Attack Order is an expert move");
        assert!(!expert.legendary);
        assert!(expert.locations.iter().any(|l| &**l == "Knot Island"));
        assert!(
            expert.condition.contains("Beedrill"),
            "got {:?}",
            expert.condition
        );

        // Plasma Fists: legendary expert (Boon Island), and its two rules join with "or".
        let plasma = dex.move_card(moves.get_id_of("PLASMAFISTS").unwrap());
        let expert = plasma.expert.expect("Plasma Fists is an expert move");
        assert!(expert.legendary);
        assert!(expert.locations.iter().any(|l| &**l == "Boon Island"));
        assert!(
            expert.condition.contains(" or "),
            "got {:?}",
            expert.condition
        );

        // Tackle isn't a signature move.
        assert!(
            dex.move_card(moves.get_id_of("TACKLE").unwrap())
                .expert
                .is_none()
        );
    }
}
