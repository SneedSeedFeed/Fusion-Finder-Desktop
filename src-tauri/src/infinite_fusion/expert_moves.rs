//! Move Expert moves: The eligibility rules live in Ruby as boolean logic in `FusionMoveTutor.rb` (`getCompatibleMoves`)
//! we scrape each `compatibleMoves << :MOVE if <expr>` rule into a small condition AST and evaluate it per fusion.
//! Conditions are built from three predicates: `is_fusion_of([...])`, `hasType(:T)`, `canLearnMove(:M)` — combined with `&&` / `||`

use std::{path::Path, sync::Arc};

use regex::Regex;
use roaring::RoaringBitmap;

use crate::infinite_fusion::{
    Dex, DexId, InfiniteFusionDex,
    filters::{HasMove, and_in, type_filter::fused_types},
    moves::{MoveDex, MoveId},
    species::{SpeciesDex, SpeciesId},
    types::{TypeDex, TypeId},
};

/// Where the Move Expert NPCs live
#[derive(Debug, Clone, Default)]
pub struct ExpertLocations {
    pub normal: Box<[Arc<str>]>,
    pub legendary: Box<[Arc<str>]>,
}

/// A boolean condition under which a fusion can be taught an expert move
#[derive(Debug, Clone)]
pub enum ExpertCond {
    /// the fusion's head or body is one of these base species
    FusionOf(Box<[SpeciesId]>),
    /// the fused typing includes this type
    HasType(TypeId),
    /// the head or body can already learn this move (level-up / tutor / egg)
    CanLearn(MoveId),
    And(Box<ExpertCond>, Box<ExpertCond>),
    Or(Box<ExpertCond>, Box<ExpertCond>),
}

/// One expert-move rule: the move, whether it's a Legendary Move Expert move, and its condition.
#[derive(Debug, Clone)]
pub struct ExpertMove {
    /// taught by the Legendary Move Expert rather than the regular one
    pub legendary: bool,
    pub move_id: MoveId,
    pub cond: ExpertCond,
}

impl ExpertCond {
    /// A plain-English description of the rule, resolving ids to names, for the move card.
    /// e.g. "a fusion of Rotom and can learn Thunder Punch", "Ghost-type or a fusion of Gastly, …".
    /// This breaks the rule I try to follow of "backend is data, frontend is presentation" but this is such a nightmare of a module idgaf.
    pub fn describe(&self, dex: &InfiniteFusionDex) -> String {
        match self {
            ExpertCond::FusionOf(list) => {
                let names: Vec<&str> = list
                    .iter()
                    .map(|&s| &*dex.species().get_item(s).name)
                    .collect();
                format!("a fusion of {}", join_or(&names))
            }
            ExpertCond::HasType(t) => format!("{}-type", titlecase(&dex.types().get_item(*t).name)),
            ExpertCond::CanLearn(m) => format!("can learn {}", dex.moves().get_item(*m).name),
            ExpertCond::And(a, b) => {
                format!(
                    "{} and {}",
                    a.describe_grouped(dex),
                    b.describe_grouped(dex)
                )
            }
            ExpertCond::Or(a, b) => format!("{} or {}", a.describe(dex), b.describe(dex)),
        }
    }

    /// Like [`describe`](Self::describe) but parenthesises an `Or` sitting under an `And`, so the grouping survives the flattening into prose.
    fn describe_grouped(&self, dex: &InfiniteFusionDex) -> String {
        match self {
            ExpertCond::Or(..) => format!("({})", self.describe(dex)),
            _ => self.describe(dex),
        }
    }
}

/// Join names as "A", "A or B", or "A, B, or C".
fn join_or(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [a] => a.to_string(),
        [a, b] => format!("{a} or {b}"),
        [rest @ .., last] => format!("{}, or {last}", rest.join(", ")),
    }
}

/// "ELECTRIC" -> "Electric".
fn titlecase(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
        None => String::new(),
    }
}

/// `FusionMoveTutor.rb` relative to the scripts dir.
const TUTOR_RB: &str = "052_InfiniteFusion/Gameplay/NPCs/FusionMoveTutor.rb";

/// `compatibleMoves << :MOVE if <expr>` — the move symbol and the condition expression.
fn rule_regex() -> Regex {
    Regex::new(r"^compatibleMoves\s*<<\s*:([A-Z][A-Z0-9_]*)\s+if\s+(.+)$").unwrap()
}

// todo! too much of the codebase can now fail silently IMO, we could do with proper logs
/// Scrape the expert-move rules out of `FusionMoveTutor.rb`.
/// a missing file, an unparseable condition, or an unknown move symbol just drops that rule instead of returning an error
pub fn collect(
    scripts_dir: &Path,
    moves: &MoveDex,
    species: &SpeciesDex,
    types: &TypeDex,
) -> Box<[ExpertMove]> {
    let Ok(text) = std::fs::read_to_string(scripts_dir.join(TUTOR_RB)) else {
        return Box::new([]);
    };
    let rule = rule_regex();

    let mut out = Vec::new();
    // the two `compatibleMoves` blocks are gated by `if !includeLegendaries` / `if includeLegendaries`.
    let mut legendary = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue; // commented-out (disabled) rule
        }
        match trimmed {
            "if !includeLegendaries" => legendary = false,
            "if includeLegendaries" => legendary = true,
            _ => {
                let Some(caps) = rule.captures(trimmed) else {
                    continue;
                };
                let Some(move_id) = moves.get_id_of(&caps[1]) else {
                    continue;
                };
                if let Some(cond) = parse_condition(&caps[2], moves, species, types) {
                    out.push(ExpertMove {
                        move_id,
                        legendary,
                        cond,
                    });
                }
            }
        }
    }
    out.into_boxed_slice()
}

#[derive(Clone, Copy)]
enum Tok<'a> {
    FusionOf,
    HasType,
    CanLearn,
    LParen,
    RParen,
    LBrack,
    RBrack,
    Comma,
    And,
    Or,
    Sym(&'a str),
}

fn tokenize(s: &str) -> Option<Vec<Tok<'_>>> {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut toks = Vec::new();
    while i < bytes.len() {
        match bytes[i] {
            b' ' | b'\t' => i += 1,
            b'(' => {
                toks.push(Tok::LParen);
                i += 1;
            }
            b')' => {
                toks.push(Tok::RParen);
                i += 1;
            }
            b'[' => {
                toks.push(Tok::LBrack);
                i += 1;
            }
            b']' => {
                toks.push(Tok::RBrack);
                i += 1;
            }
            b',' => {
                toks.push(Tok::Comma);
                i += 1;
            }
            b'&' if bytes.get(i + 1) == Some(&b'&') => {
                toks.push(Tok::And);
                i += 2;
            }
            b'|' if bytes.get(i + 1) == Some(&b'|') => {
                toks.push(Tok::Or);
                i += 2;
            }
            b':' => {
                let start = i + 1;
                let end = ident_end(bytes, start);
                if end == start {
                    return None;
                }
                toks.push(Tok::Sym(&s[start..end]));
                i = end;
            }
            c if c.is_ascii_alphabetic() || c == b'_' => {
                let end = ident_end(bytes, i);
                let tok = match &s[i..end] {
                    "is_fusion_of" => Tok::FusionOf,
                    "hasType" => Tok::HasType,
                    "canLearnMove" => Tok::CanLearn,
                    _ => return None, // unknown predicate -> skip the whole rule
                };
                toks.push(tok);
                i = end;
            }
            _ => return None,
        }
    }
    Some(toks)
}

fn ident_end(bytes: &[u8], start: usize) -> usize {
    let mut j = start;
    while j < bytes.len() && (bytes[j].is_ascii_alphanumeric() || bytes[j] == b'_') {
        j += 1;
    }
    j
}

/// Recursive-descent parser resolving symbols against the dexes as it goes.
/// Returns `None` if the expression is malformed or a required `hasType` / `canLearnMove` symbol doesn't resolve in this game's data.
struct Parser<'a> {
    toks: &'a [Tok<'a>],
    pos: usize,
    moves: &'a MoveDex,
    species: &'a SpeciesDex,
    types: &'a TypeDex,
}

fn parse_condition(
    expr: &str,
    moves: &MoveDex,
    species: &SpeciesDex,
    types: &TypeDex,
) -> Option<ExpertCond> {
    let toks = tokenize(expr)?;
    let mut parser = Parser {
        toks: &toks,
        pos: 0,
        moves,
        species,
        types,
    };
    let cond = parser.parse_or()?;
    // a trailing token means we didn't consume the whole expression -> treat as malformed
    (parser.pos == toks.len()).then_some(cond)
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<Tok<'a>> {
        self.toks.get(self.pos).copied()
    }

    fn eat(&mut self, want: impl Fn(Tok<'a>) -> bool) -> Option<()> {
        matches!(self.peek(), Some(t) if want(t)).then(|| self.pos += 1)
    }

    fn parse_or(&mut self) -> Option<ExpertCond> {
        let mut left = self.parse_and()?;
        while matches!(self.peek(), Some(Tok::Or)) {
            self.pos += 1;
            let right = self.parse_and()?;
            left = ExpertCond::Or(Box::new(left), Box::new(right));
        }
        Some(left)
    }

    fn parse_and(&mut self) -> Option<ExpertCond> {
        let mut left = self.parse_term()?;
        while matches!(self.peek(), Some(Tok::And)) {
            self.pos += 1;
            let right = self.parse_term()?;
            left = ExpertCond::And(Box::new(left), Box::new(right));
        }
        Some(left)
    }

    fn parse_term(&mut self) -> Option<ExpertCond> {
        match self.peek()? {
            Tok::LParen => {
                self.pos += 1;
                let inner = self.parse_or()?;
                self.eat(|t| matches!(t, Tok::RParen))?;
                Some(inner)
            }
            Tok::FusionOf => self.parse_fusion_of(),
            Tok::HasType => {
                self.pos += 1;
                let sym = self.paren_sym()?;
                Some(ExpertCond::HasType(self.types.get_id_of(sym)?))
            }
            Tok::CanLearn => {
                self.pos += 1;
                let sym = self.paren_sym()?;
                Some(ExpertCond::CanLearn(self.moves.get_id_of(sym)?))
            }
            _ => None,
        }
    }

    /// `(:SYMBOL)`
    fn paren_sym(&mut self) -> Option<&'a str> {
        self.eat(|t| matches!(t, Tok::LParen))?;
        let Some(Tok::Sym(s)) = self.peek() else {
            return None;
        };
        self.pos += 1;
        self.eat(|t| matches!(t, Tok::RParen))?;
        Some(s)
    }

    /// `([:A, :B, ...])` with unknown species dropped
    fn parse_fusion_of(&mut self) -> Option<ExpertCond> {
        self.pos += 1; // FusionOf
        self.eat(|t| matches!(t, Tok::LParen))?;
        self.eat(|t| matches!(t, Tok::LBrack))?;
        let mut list = Vec::new();
        while let Some(Tok::Sym(s)) = self.peek() {
            self.pos += 1;
            if let Some(id) = self.species.get_id_of(s) {
                list.push(id);
            }
            if !matches!(self.peek(), Some(Tok::Comma)) {
                break;
            }
            self.pos += 1;
        }
        self.eat(|t| matches!(t, Tok::RBrack))?;
        self.eat(|t| matches!(t, Tok::RParen))?;
        Some(ExpertCond::FusionOf(list.into_boxed_slice()))
    }
}

impl InfiniteFusionDex {
    /// The expert moves a `head`/`body` fusion qualifies for, each flagged legendary or not
    pub fn expert_moves_for(&self, head: SpeciesId, body: SpeciesId) -> Vec<(MoveId, bool)> {
        self.expert_moves
            .iter()
            .filter(|em| self.eval_expert_cond(&em.cond, head, body))
            .map(|em| (em.move_id, em.legendary))
            .collect()
    }

    fn eval_expert_cond(&self, cond: &ExpertCond, head: SpeciesId, body: SpeciesId) -> bool {
        match cond {
            ExpertCond::FusionOf(list) => list.iter().any(|&s| s == head || s == body),
            ExpertCond::HasType(t) => {
                let (t1, t2) = fused_types(
                    self.species().get_item(head),
                    self.species().get_item(body),
                    self.types(),
                );
                t1 == *t || t2 == Some(*t)
            }
            ExpertCond::CanLearn(m) => {
                self.move_index().learns_any(*m, head) || self.move_index().learns_any(*m, body)
            }
            ExpertCond::And(a, b) => {
                self.eval_expert_cond(a, head, body) && self.eval_expert_cond(b, head, body)
            }
            ExpertCond::Or(a, b) => {
                self.eval_expert_cond(a, head, body) || self.eval_expert_cond(b, head, body)
            }
        }
    }

    /// Per-head body set for the move filter: bodies whose fusion with `head` learns each requested move via the enabled sources.
    /// `None` = no constraint (every body qualifies).
    pub fn move_bodies_for_head_with_expert(
        &self,
        head: SpeciesId,
        has_move: &HasMove,
    ) -> Option<RoaringBitmap> {
        let mut acc: Option<RoaringBitmap> = None;
        for &move_id in &has_move.moves {
            // normal sources: if the head learns it, no constraint; otherwise the body must.
            let normal =
                self.move_index()
                    .learners(move_id, has_move.egg, has_move.level, has_move.tutor);
            let mut per_move = (!normal.contains(head.to_u32())).then_some(normal);
            if has_move.expert {
                per_move = or_bodies(per_move, self.expert_bodies_for_head(head, move_id));
            }
            // `None` here means this move constrains nothing (the head already covers it).
            if let Some(set) = per_move {
                and_in(&mut acc, set);
            }
        }
        acc
    }

    /// Bodies whose fusion with `head` is eligible for expert move `move_id`, across all its rules.
    /// `None` = every body qualifies (the head alone satisfies a rule).
    fn expert_bodies_for_head(&self, head: SpeciesId, move_id: MoveId) -> Option<RoaringBitmap> {
        let mut out: Option<RoaringBitmap> = Some(RoaringBitmap::new());
        for em in self.expert_moves.iter().filter(|em| em.move_id == move_id) {
            out = or_bodies(out, self.cond_bodies(&em.cond, head));
            if out.is_none() {
                break; // already "all bodies"
            }
        }
        out
    }

    /// Evaluate a condition to the per-head set of qualifying bodies. `None` = all bodies
    fn cond_bodies(&self, cond: &ExpertCond, head: SpeciesId) -> Option<RoaringBitmap> {
        match cond {
            // head is one of the species -> every fusion qualifies; else the body must be
            ExpertCond::FusionOf(list) => {
                (!list.contains(&head)).then(|| list.iter().map(|&s| s.to_u32()).collect())
            }
            ExpertCond::HasType(t) => self.type_index().bodies_for_head(head, &[*t]),
            ExpertCond::CanLearn(m) => (!self.move_index().learns_any(*m, head))
                .then(|| self.move_index().learners(*m, true, true, true)),
            ExpertCond::And(a, b) => {
                and_bodies(self.cond_bodies(a, head), self.cond_bodies(b, head))
            }
            ExpertCond::Or(a, b) => or_bodies(self.cond_bodies(a, head), self.cond_bodies(b, head)),
        }
    }
}

/// Intersect two body sets where `None` means "all bodies"
fn and_bodies(a: Option<RoaringBitmap>, b: Option<RoaringBitmap>) -> Option<RoaringBitmap> {
    match (a, b) {
        (Some(a), Some(b)) => Some(&a & &b),
        (set, None) | (None, set) => set,
    }
}

/// Union two body sets where `None` means "all bodies" (so it dominates)
fn or_bodies(a: Option<RoaringBitmap>, b: Option<RoaringBitmap>) -> Option<RoaringBitmap> {
    match (a, b) {
        (Some(a), Some(b)) => Some(&a | &b),
        _ => None,
    }
}

#[cfg(test)]
mod test {
    use crate::{
        infinite_fusion::{Dex, GameVersion, InfiniteFusionDex},
        test::infinite_fusion_dir,
    };

    #[test]
    fn evaluates_signature_move_conditions() {
        let dex = InfiniteFusionDex::from_path(infinite_fusion_dir(), GameVersion::Kanto).unwrap();
        let sp = |s: &str| dex.species().get_id_of(s).unwrap();
        let mv = |m: &str| dex.moves().get_id_of(m).unwrap();
        let teaches = |head, body, move_id| {
            dex.expert_moves_for(head, body)
                .iter()
                .find(|&&(m, _)| m == move_id)
                .map(|&(_, legendary)| legendary)
        };

        // is_fusion_of([:BEEDRILL]) -> Attack Order, from the regular expert.
        assert_eq!(
            teaches(sp("BEEDRILL"), sp("BEEDRILL"), mv("ATTACKORDER")),
            Some(false)
        );
        // a fusion of neither side never gets it.
        assert_eq!(
            teaches(sp("BULBASAUR"), sp("PIDGEY"), mv("ATTACKORDER")),
            None
        );

        // is_fusion_of([:ELECTABUZZ, ...]) -> Plasma Fists, from the legendary expert.
        assert_eq!(
            teaches(sp("ELECTABUZZ"), sp("PIDGEY"), mv("PLASMAFISTS")),
            Some(true)
        );
        // the other Plasma Fists branch: is_fusion_of([:ROTOM]) && canLearnMove(:THUNDERPUNCH). Rotom alone cannot so fuse with Electabuzz so it qualifies.
        assert_eq!(teaches(sp("ROTOM"), sp("ROTOM"), mv("PLASMAFISTS")), None);
        assert_eq!(
            teaches(sp("ROTOM"), sp("ELECTABUZZ"), mv("PLASMAFISTS")),
            Some(true)
        );
    }
}
