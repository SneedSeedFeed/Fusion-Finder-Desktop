//! Rich per-fusion detail for the inspect view. Gives fused types, stats vs. each parent, abilities,
//! type matchups, both parents' evol lines + where to get them, and the combined move pool.

use std::sync::Arc;

use serde::Serialize;

use crate::infinite_fusion::{
    Dex, DexId, InfiniteFusionDex,
    encounters::{EncounterMethod, EncounterMode},
    filters::type_filter::fused_types,
    moves::{MoveCategory, MoveId},
    species::{SpeciesId, evolution::EvolutionKind},
    types::TypeId,
};

#[derive(Debug, Clone, Serialize)]
pub struct FusionDetail {
    pub head: ComponentBrief,
    pub body: ComponentBrief,
    pub fusion_name: FusionName,
    /// the fusion's (primary, optional secondary) type ids — resolve via the bootstrap `types` table
    pub types: (TypeId, Option<TypeId>),
    pub stats: (
        StatRow,
        StatRow,
        StatRow,
        StatRow,
        StatRow,
        StatRow,
        StatRow<u16>,
    ),
    pub regular_abilities: Box<[NamedDesc]>,
    pub hidden_abilities: Box<[NamedDesc]>,
    /// defensive matchups grouped by multiplier, strongest first; only non-1× groups
    pub matchups: Box<[Matchup]>,
    /// the fusion's own evolution neighbours
    pub evolves_from: Box<[FusionEvo]>,
    pub evolves_into: Box<[FusionEvo]>,
    /// evolution family of each parent, base-first, with where each member is found
    pub head_line: Box<[EvoNode]>,
    pub body_line: Box<[EvoNode]>,
    pub moves: Box<[MoveRow]>,
    /// available custom sprite variants + attribution (filled by the command, may be empty)
    pub sprites: Box<[SpriteVariant]>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComponentBrief {
    /// our internal species index
    pub index: SpeciesId,
    /// in-game dex number (for sprite urls)
    pub dex_id: u16,
    pub name: Box<str>,
}

/// One fused stat vs. each parent. No label — the front end knows the 7 stats by position (the
/// order of `STATS`: HP, ATK, DEF, SP.ATK, SP.DEF, SPEED, TOTAL).
#[derive(Debug, Clone, Serialize)]
pub struct StatRow<T = u8> {
    pub value: T,
    pub head: T,
    pub body: T,
}

#[derive(Debug, Clone, Serialize)]
pub struct NamedDesc {
    pub name: Box<str>,
    pub description: Box<str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Matchup {
    /// effectiveness in quarters (0 = immune, 1 = ¼×, 2 = ½×, 8 = 2×, 16 = 4×); the front end
    /// turns this into the ×-glyph
    pub multiplier: u8,
    /// attacking type ids in this multiplier group (resolve via the bootstrap `types` table)
    pub types: Box<[TypeId]>,
}

/// Which half of the fusion takes the evolution step.
#[derive(Debug, Clone, Copy, Serialize)]
pub enum Component {
    Head,
    Body,
}

/// A fusion reachable by evolving (or de-evolving) one component of the current fusion.
#[derive(Debug, Clone, Serialize)]
pub struct FusionEvo {
    /// species indices (for navigating the inspector to it)
    pub head: SpeciesId,
    pub body: SpeciesId,
    /// in-game dex numbers (for the sprite url)
    pub head_dex: u16,
    pub body_dex: u16,
    pub name: FusionName,
    /// which component changes
    pub via: Component,
    /// how it gets there; `None` for a de-evolution
    pub condition: Option<EvoCondition>,
}

/// The qualifier on a level-up evolution (time of day / stat relation), if any.
#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LevelNote {
    None,
    Day,
    Night,
    AtkGtDef,
    AtkLtDef,
    AtkEqDef,
}

/// How a fusion neighbour is reached — structured data; the front end composes the sentence.
/// Item/move ids are resolved to names here (the front end has no items table).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EvoCondition {
    Level { level: u8, note: LevelNote },
    UseItem { item: Box<str> },
    HoldDay { item: Box<str> },
    Trade { item: Box<str> },
    KnowMove { name: Box<str> },
}

#[derive(Debug, Clone, Serialize)]
pub struct EvoNode {
    pub dex_id: u16,
    pub name: Box<str>,
    pub types: (TypeId, Option<TypeId>),
    /// how this member is reached from the previous one in the line, if any
    pub from_condition: Option<EvoCondition>,
    pub encounters: Box<[EncounterRow]>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EncounterRow {
    pub location: Arc<str>,
    pub method: EncounterMethod,
    pub chance: u8,
    pub min_level: u8,
    pub max_level: u8,
    /// which game mode this wild encounter belongs to (scripted encounters are always [`EncounterMode::Both`])
    pub mode: EncounterMode,
}

/// How a move is learned, merged across both parents
#[derive(Debug, Clone, Serialize)]
pub struct LearnSources {
    /// level it's learned at (`Some(0)` = on evolution); `None` if not a level-up move
    pub level: Option<u8>,
    pub machine: Option<Box<str>>,
    /// learnable from a (non-machine) move tutor
    pub tutor: bool,
    pub egg: bool,
    /// a Move Expert signature move for this fusion:
    /// `Some(true)` = legendary expert, `Some(false)` = regular expert, `None` = not an expert move.
    pub expert: Option<bool>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MoveRow {
    pub id: MoveId,
    pub name: Box<str>,
    pub ty: TypeId,
    pub category: MoveCategory,
    pub power: Option<u8>,
    /// `None` = never misses
    pub accuracy: Option<u8>,
    pub pp: u8,
    pub description: Box<str>,
    pub sources: LearnSources,
}

#[derive(Debug, Clone)]
pub struct FusionName {
    pub first_half: Arc<str>,
    pub second_half: Arc<str>,
}

impl std::fmt::Display for FusionName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.first_half)?;
        f.write_str(&self.second_half)
    }
}

impl Serialize for FusionName {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(self)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct SpriteVariant {
    /// "" for the base sprite, else the letter suffix ("a", "b", …)
    pub variant: Box<str>,
    pub artist: Option<Box<str>>,
}

macro_rules! get_stats {
    ($fused:ident, $h:ident, $b:ident, [$($stat:ident),*]) => {
        {
            ($(StatRow { value: $fused.$stat(), head: $h.base_stats.$stat(), body: $b.base_stats.$stat() }),*)
        }
    };
}

impl InfiniteFusionDex {
    /// Build the inspect payload for fusion `head`/`body`. `sprites` is left empty for the command
    /// layer (which owns the sprite manifest/credits) to fill.
    pub fn fusion_detail(&self, head: SpeciesId, body: SpeciesId) -> FusionDetail {
        let h = self.species().get_item(head);
        let b = self.species().get_item(body);

        let (t1, t2) = fused_types(h, b, self.types());

        let stats = if head != body {
            let fused = h.base_stats.fuse(&b.base_stats);
            get_stats!(fused, h, b, [hp, atk, def, spa, spd, spe, bst])
        } else {
            let fused = h.base_stats;
            get_stats!(fused, h, b, [hp, atk, def, spa, spd, spe, bst])
        };

        FusionDetail {
            head: self.brief(head),
            body: self.brief(body),
            fusion_name: FusionName {
                first_half: h.names.first_half.clone(),
                second_half: b.names.second_half.clone(),
            },
            types: (t1, t2),
            stats,
            regular_abilities: self.abilities_of(&[&h.abilities, &b.abilities]),
            hidden_abilities: self.abilities_of(&[&h.hidden_abilities, &b.hidden_abilities]),
            matchups: self.matchups(t1, t2),
            evolves_from: self.fusion_evos(head, body, false),
            evolves_into: self.fusion_evos(head, body, true),
            head_line: self.evo_line(head),
            body_line: self.evo_line(body),
            moves: self.fusion_moves(head, body, h, b),
            sprites: Box::default(),
        }
    }

    fn brief(&self, id: SpeciesId) -> ComponentBrief {
        let s = self.species().get_item(id);
        ComponentBrief {
            index: id,
            dex_id: s.id_number,
            name: s.name.clone(),
        }
    }

    /// Unique abilities drawn from the given slot lists (head then body), with descriptions.
    fn abilities_of(
        &self,
        slots: &[&[crate::infinite_fusion::abilities::AbilityId]],
    ) -> Box<[NamedDesc]> {
        let mut seen = Vec::new();
        let mut out = Vec::new();
        for &ability in slots.iter().flat_map(|s| s.iter()) {
            if seen.contains(&ability) {
                continue;
            }
            seen.push(ability);
            let a = self.abilities().get_item(ability);
            out.push(NamedDesc {
                name: a.name.clone(),
                description: a.description.clone(),
            });
        }
        out.into_boxed_slice()
    }

    /// Defensive matchups for the fused type pair, grouped by combined multiplier (skipping 1×).
    fn matchups(&self, t1: TypeId, t2: Option<TypeId>) -> Box<[Matchup]> {
        use itertools::Itertools;

        let defenders = [Some(t1), t2];

        // combined effectiveness of `atk` against the fused pair, in quarters i.e 4 is 1x
        let quarters = |atk: TypeId| {
            defenders
                .iter()
                .flatten()
                .fold(4u32, |q, &def| {
                    let d = self.types().get_item(def);
                    let factor = if d.immunities.contains(atk) {
                        0
                    } else if d.weaknesses.contains(atk) {
                        8
                    } else if d.resistances.contains(atk) {
                        2
                    } else {
                        4
                    };
                    q * factor / 4
                })
                .min(255) as u8
        };

        (0..self.types().len())
            .map(TypeId::from_usize)
            .filter(|&atk| self.types().get(atk).0 != "???") // skip the placeholder type
            .map(|atk| (quarters(atk), atk))
            .filter(|&(q, _)| q != 4) // drop neutral matchups
            .sorted_by(|a, b| b.0.cmp(&a.0)) // strongest first
            .chunk_by(|&(q, _)| q)
            .into_iter()
            .map(|(multiplier, group)| Matchup {
                multiplier,
                types: group.map(|(_, atk)| atk).collect(),
            })
            .collect()
    }

    /// The evolution family containing `species`, ordered base-first, each node carrying the condition to reach it and where it's found in the wild.
    fn evo_line(&self, species: SpeciesId) -> Box<[EvoNode]> {
        let chain = self.evo_chain(species);
        chain
            .iter()
            .enumerate()
            .map(|(i, &id)| {
                let s = self.species().get_item(id);
                let types = (s.type1, s.type2);
                let from_condition = (i > 0)
                    .then(|| self.evo_condition(chain[i - 1], id))
                    .flatten();
                EvoNode {
                    dex_id: s.id_number,
                    name: s.name.clone(),
                    types,
                    from_condition,
                    encounters: self.encounter_rows(id),
                }
            })
            .collect()
    }

    /// Linear evolution chain (pre-evos + species + forward evos), de-duplicated.
    /// Branches follow the first listed evolution.
    fn evo_chain(&self, species: SpeciesId) -> Vec<SpeciesId> {
        let mut chain = vec![species];

        // walk backwards to the base
        let mut cur = species;
        while let Some(prev) = self.pre_evo(cur) {
            if chain.contains(&prev) {
                break;
            }
            chain.insert(0, prev);
            cur = prev;
        }

        // walk forwards along evolutions
        let mut cur = species;
        while let Some(next) = self.next_evo(cur) {
            if chain.contains(&next) {
                break;
            }
            chain.push(next);
            cur = next;
        }

        chain
    }

    /// The fusion's evolution neighbours: each is the current fusion with one component stepped one
    /// rung up (`forward`) or down its evolution line. Mirrors how Infinite Fusion evolves fusions
    /// (either component can evolve independently).
    fn fusion_evos(&self, head: SpeciesId, body: SpeciesId, forward: bool) -> Box<[FusionEvo]> {
        let mut out = Vec::new();
        // head steps
        for (next, kind) in self.component_steps(head, forward) {
            out.push(self.make_fusion_evo(next, body, Component::Head, kind));
        }
        // body steps
        for (next, kind) in self.component_steps(body, forward) {
            out.push(self.make_fusion_evo(head, next, Component::Body, kind));
        }
        out.into_boxed_slice()
    }

    /// One rung along `species`' evolution line: forward evolutions (with their kind) or
    /// pre-evolutions (no kind — it's a de-evolution).
    fn component_steps(
        &self,
        species: SpeciesId,
        forward: bool,
    ) -> Vec<(SpeciesId, Option<EvolutionKind>)> {
        if forward {
            self.species()
                .get_item(species)
                .evolutions
                .iter()
                .filter(|e| e.target().is_into())
                .map(|e| (e.target().species(), Some(e.kind())))
                .collect()
        } else {
            // explicit `from` links, else whoever evolves into us
            let froms: Vec<_> = self
                .species()
                .get_item(species)
                .evolutions
                .iter()
                .filter(|e| !e.target().is_into())
                .map(|e| (e.target().species(), None))
                .collect();
            if !froms.is_empty() {
                return froms;
            }
            (0..self.species().len())
                .map(SpeciesId::from_usize)
                .filter(|&id| {
                    self.species()
                        .get_item(id)
                        .evolutions
                        .iter()
                        .any(|e| e.target().is_into() && e.target().species() == species)
                })
                .map(|id| (id, None))
                .collect()
        }
    }

    fn make_fusion_evo(
        &self,
        head: SpeciesId,
        body: SpeciesId,
        via: Component,
        kind: Option<EvolutionKind>,
    ) -> FusionEvo {
        let h = self.species().get_item(head);
        let b = self.species().get_item(body);
        FusionEvo {
            head,
            body,
            head_dex: h.id_number,
            body_dex: b.id_number,
            name: FusionName {
                first_half: h.names.first_half.clone(),
                second_half: b.names.second_half.clone(),
            },
            via,
            condition: kind.map(|k| self.simplify_evo_kind(k)),
        }
    }

    fn pre_evo(&self, species: SpeciesId) -> Option<SpeciesId> {
        // explicit `from` link first
        if let Some(from) = self
            .species()
            .get_item(species)
            .evolutions
            .iter()
            .find(|e| !e.target().is_into())
        {
            return Some(from.target().species());
        }
        // else find whoever evolves into us
        (0..self.species().len())
            .map(SpeciesId::from_usize)
            .find(|&id| {
                self.species()
                    .get_item(id)
                    .evolutions
                    .iter()
                    .any(|e| e.target().is_into() && e.target().species() == species)
            })
    }

    fn next_evo(&self, species: SpeciesId) -> Option<SpeciesId> {
        self.species()
            .get_item(species)
            .evolutions
            .iter()
            .find(|e| e.target().is_into())
            .map(|e| e.target().species())
    }

    /// Structured condition for `from` evolving into `to`, if such an edge exists.
    fn evo_condition(&self, from: SpeciesId, to: SpeciesId) -> Option<EvoCondition> {
        let edge = self
            .species()
            .get_item(from)
            .evolutions
            .iter()
            .find(|e| e.target().is_into() && e.target().species() == to)?;
        Some(self.simplify_evo_kind(edge.kind()))
    }

    /// Simplify the sent evolution kind from the deserialized form
    fn simplify_evo_kind(&self, kind: EvolutionKind) -> EvoCondition {
        let item = |id| self.items().get_item(id).name.clone();
        let mv = |id| self.moves().get_item(id).name.clone();
        use EvolutionKind as K;
        match kind {
            K::Level { level }
            | K::Shedinja { level }
            | K::Ninjask { level }
            | K::Silcoon { level }
            | K::Cascoon { level } => EvoCondition::Level {
                level,
                note: LevelNote::None,
            },
            K::LevelDay { level } => EvoCondition::Level {
                level,
                note: LevelNote::Day,
            },
            K::LevelNight { level } => EvoCondition::Level {
                level,
                note: LevelNote::Night,
            },
            K::AttackGreater { level } => EvoCondition::Level {
                level,
                note: LevelNote::AtkGtDef,
            },
            K::DefenseGreater { level } => EvoCondition::Level {
                level,
                note: LevelNote::AtkLtDef,
            },
            K::AtkDefEqual { level } => EvoCondition::Level {
                level,
                note: LevelNote::AtkEqDef,
            },
            K::Item { item_id } => EvoCondition::UseItem {
                item: item(item_id),
            },
            K::DayHoldItem { item_id } => EvoCondition::HoldDay {
                item: item(item_id),
            },
            K::TradeItem { item_id } => EvoCondition::Trade {
                item: item(item_id),
            },
            K::HasMove { move_id } => EvoCondition::KnowMove { name: mv(move_id) },
        }
    }

    fn encounter_rows(&self, species: SpeciesId) -> Box<[EncounterRow]> {
        let mut rows: Vec<EncounterRow> = self
            .encounters()
            .for_species(species)
            .iter()
            .map(|e| EncounterRow {
                location: e.route.clone(),
                method: e.method,
                chance: e.chance,
                min_level: e.min_level,
                max_level: e.max_level,
                mode: e.mode,
            })
            .collect();
        // group by mode (Both first, then Classic, then Remix) then location for a stable display
        rows.sort_by(|a, b| {
            mode_order(a.mode)
                .cmp(&mode_order(b.mode))
                .then_with(|| a.location.cmp(&b.location))
        });
        rows.into_boxed_slice()
    }

    /// The fusion's move pool: the union of both parents', plus any Move Expert signature moves it
    /// qualifies for, tagged with how each is learned.
    fn fusion_moves(
        &self,
        head_id: SpeciesId,
        body_id: SpeciesId,
        head: &crate::infinite_fusion::species::SpeciesDetails,
        body: &crate::infinite_fusion::species::SpeciesDetails,
    ) -> Box<[MoveRow]> {
        use std::collections::HashMap;

        // move id -> (min level if a level-up move, learnable by tutor, learnable as egg move)
        let mut acc: HashMap<MoveId, (Option<u8>, bool, bool)> = HashMap::new();
        for s in [head, body] {
            for lm in s.moves.iter() {
                let entry = acc.entry(lm.move_id()).or_default();
                entry.0 = Some(entry.0.map_or(lm.level(), |cur| cur.min(lm.level())));
            }
            for &m in s.tutor_moves.iter() {
                acc.entry(m).or_default().1 = true;
            }
            for &m in s.egg_moves.iter() {
                acc.entry(m).or_default().2 = true;
            }
        }

        let rows: Vec<MoveRow> = acc
            .into_iter()
            .map(|(id, (level, tutor, egg))| {
                let m = self.moves().get_item(id);
                // `tutor_moves` lumps TM/HM and genuine move-tutor moves together; if a machine
                // teaches this move it's a machine source, otherwise a real move tutor.
                let machine = if tutor {
                    self.machine_for_move(id).map(Box::from)
                } else {
                    None
                };
                MoveRow {
                    id,
                    name: m.name.clone(),
                    ty: m.ty,
                    category: m.category,
                    power: m.power.map(|p| p.get()),
                    accuracy: m.accuracy.percent(),
                    pp: m.pp,
                    description: m.description.clone(),
                    sources: LearnSources {
                        level,
                        tutor: tutor && machine.is_none(),
                        machine,
                        egg,
                        expert: None,
                    },
                }
            })
            .collect();

        // Collapse moves that share a display name: the game data carries aliased duplicates (e.g.
        // both FEINTATTACK and FAINTATTACK render as "Feint Attack"), which would otherwise show as
        // two rows and collide on the front end's name-keyed list. Merge their sources and keep the
        // lowest level for ordering.
        let mut by_name: HashMap<Box<str>, usize> = HashMap::new();
        let mut merged: Vec<MoveRow> = Vec::with_capacity(rows.len());
        for row in rows {
            if let Some(&idx) = by_name.get(&row.name) {
                let s = &mut merged[idx].sources;
                let r = row.sources;
                s.level = match (s.level, r.level) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (a, b) => a.or(b),
                };
                s.machine = s.machine.take().or(r.machine);
                s.tutor |= r.tutor;
                s.egg |= r.egg;
                s.expert = s.expert.or(r.expert);
            } else {
                by_name.insert(row.name.clone(), merged.len());
                merged.push(row);
            }
        }

        // overlay the Move Expert signature moves this fusion qualifies for
        let mut experts: HashMap<MoveId, bool> = self
            .expert_moves_for(head_id, body_id)
            .into_iter()
            .collect();
        for row in &mut merged {
            if let Some(legendary) = experts.remove(&row.id) {
                row.sources.expert = Some(legendary);
            }
        }
        for (id, legendary) in experts {
            let m = self.moves().get_item(id);
            merged.push(MoveRow {
                id,
                name: m.name.clone(),
                ty: m.ty,
                category: m.category,
                power: m.power.map(|p| p.get()),
                accuracy: m.accuracy.percent(),
                pp: m.pp,
                description: m.description.clone(),
                sources: LearnSources {
                    level: None,
                    machine: None,
                    tutor: false,
                    egg: false,
                    expert: Some(legendary),
                },
            });
        }

        // level-up moves first (by level), then the rest; alphabetical within
        merged.sort_by(|a, b| match (a.sources.level, b.sources.level) {
            (Some(x), Some(y)) => x.cmp(&y).then_with(|| a.name.cmp(&b.name)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.name.cmp(&b.name),
        });
        merged.into()
    }
}

/// Display order for the encounter mode groups: shared encounters first, then mode-exclusive ones.
fn mode_order(mode: EncounterMode) -> u8 {
    match mode {
        EncounterMode::Both => 0,
        EncounterMode::Classic => 1,
        EncounterMode::Remix => 2,
    }
}

#[cfg(test)]
mod test {
    use super::{Component, EvoCondition};
    use crate::{
        infinite_fusion::{Dex, GameVersion, InfiniteFusionDex, encounters::EncounterMethod},
        test::{infinite_fusion_dir, infinite_fusion_hoenn_dir},
    };

    #[test]
    fn dugtrio_deoxys_detail() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let dugtrio = dex.species().get_id_of("DUGTRIO").unwrap();
        let deoxys = dex.species().get_id_of("DEOXYS").unwrap();
        let d = dex.fusion_detail(dugtrio, deoxys);

        // Ground (head) / Psychic (body)
        let ground = dex.types().get_id_of("GROUND").unwrap();
        let psychic = dex.types().get_id_of("PSYCHIC").unwrap();
        assert_eq!(d.types, (ground, Some(psychic)));
        assert!(!d.regular_abilities.is_empty());

        // head line is Diglett -> Dugtrio with a level-up condition
        assert!(d.head_line.iter().any(|n| &*n.name == "Diglett"));
        let dug = d.head_line.iter().find(|n| &*n.name == "Dugtrio").unwrap();
        assert!(matches!(
            dug.from_condition,
            Some(EvoCondition::Level { .. })
        ));

        // combined move pool includes a Dugtrio move
        assert!(d.moves.iter().any(|m| &*m.name == "Sand Tomb"));

        // machine moves are labelled with their TM/HM (not the generic "Tutor"): this fusion can
        // learn at least one move via a TM.
        assert!(
            d.moves.iter().any(|m| m
                .sources
                .machine
                .as_deref()
                .is_some_and(|tm| tm.starts_with("TM"))),
            "expected at least one TM-sourced move in the pool"
        );

        // Ground/Psychic is immune to Electric (Ground immunity) — 0 quarters = ×0
        let electric = dex.types().get_id_of("ELECTRIC").unwrap();
        assert!(
            d.matchups
                .iter()
                .any(|m| m.multiplier == 0 && m.types.contains(&electric))
        );

        // Dugtrio de-evolves to Diglett, so the fusion evolves *from* Diglett/Deoxys (via Head);
        // neither Dugtrio nor Deoxys evolves further, so there's nothing to evolve into.
        let diglett = dex.species().get_id_of("DIGLETT").unwrap();
        assert!(
            d.evolves_from
                .iter()
                .any(|e| e.head == diglett && matches!(e.via, Component::Head))
        );
        assert!(d.evolves_into.is_empty());
    }

    #[test]
    fn move_pool_includes_expert_signature_moves() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let sp = |s: &str| dex.species().get_id_of(s).unwrap();

        // A Beedrill fusion gets Attack Order from the regular expert, tagged accordingly.
        let bee = dex.fusion_detail(sp("BEEDRILL"), sp("PIDGEY"));
        let attack_order = bee.moves.iter().find(|m| &*m.name == "Attack Order");
        assert_eq!(attack_order.map(|m| m.sources.expert), Some(Some(false)));

        // An Electabuzz fusion gets Plasma Fists from the legendary expert.
        let elec = dex.fusion_detail(sp("ELECTABUZZ"), sp("PIDGEY"));
        let plasma = elec.moves.iter().find(|m| &*m.name == "Plasma Fists");
        assert_eq!(plasma.map(|m| m.sources.expert), Some(Some(true)));

        // A fusion that qualifies for neither doesn't gain them.
        let bulba = dex.fusion_detail(sp("BULBASAUR"), sp("PIDGEY"));
        assert!(!bulba.moves.iter().any(|m| &*m.name == "Attack Order"));
    }

    #[test]
    fn move_pool_has_no_duplicate_names() {
        // Absol/Dragalge both reach "Feint Attack" via the game's aliased FEINTATTACK/FAINTATTACK
        // move symbols; the pool must collapse them to a single row (the front end keys moves by
        // name, so a dupe would crash it). Dragalge is Hoenn-only, which is where this was hit.
        let dex =
            InfiniteFusionDex::from_path(infinite_fusion_hoenn_dir(), GameVersion::Hoenn).unwrap();
        let absol = dex.species().get_id_of("ABSOL").unwrap();
        let dragalge = dex.species().get_id_of("DRAGALGE").unwrap();

        let detail = dex.fusion_detail(absol, dragalge);

        let mut names: Vec<&str> = detail.moves.iter().map(|m| &*m.name).collect();
        names.sort_unstable();
        let unique = names.len();
        names.dedup();
        assert_eq!(unique, names.len(), "duplicate move names in the pool");
        assert!(detail.moves.iter().any(|m| &*m.name == "Feint Attack"));
    }

    /// End-to-end across both games: building the dex runs the full pipeline (wild encounters plus
    /// the static/gift rows scraped from map events and the roaming/radar/starter rows from
    /// Settings), so this is the one place `from_path` runs for Hoenn.
    #[test]
    fn builds_both_games_with_scraped_encounters() {
        for (dir, version) in [
            (infinite_fusion_dir(), GameVersion::Kanto),
            (infinite_fusion_hoenn_dir(), GameVersion::Hoenn),
        ] {
            let dex = InfiniteFusionDex::from_path(dir, version).unwrap();

            // The scraped, non-wild acquisition rows are merged into the encounter table.
            assert!(
                dex.encounters().all().iter().any(|e| matches!(
                    e.method,
                    EncounterMethod::Static
                        | EncounterMethod::Gift
                        | EncounterMethod::Roaming
                        | EncounterMethod::PokeRadar
                )),
                "{version:?}: expected scraped static/gift/roaming/radar encounters"
            );
        }
    }
}
