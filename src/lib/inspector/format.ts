// Presentation helpers for the inspect view. The backend sends structured data (enums, numbers,
// ids resolved to names); composing the display strings is the front end's job — these live here.
import type { EvoCondition, LearnSources } from "$lib/bindings";

// stat row labels, by position (matches the backend `STATS` order)
export const STAT_LABELS = [
  "HP",
  "ATK",
  "DEF",
  "SP.ATK",
  "SP.DEF",
  "SPEED",
  "TOTAL",
] as const;

// type-effectiveness glyph from the backend's quarter-units (16 = 4×, 8 = 2×, 2 = ½×, …)
const MULTIPLIER_GLYPH: Record<number, string> = {
  0: "0",
  1: "¼",
  2: "½",
  8: "2",
  16: "4",
};
export function multiplierGlyph(quarters: number): string {
  return MULTIPLIER_GLYPH[quarters] ?? String(quarters / 4);
}

// EncounterMethod variant name -> player-facing label
const METHOD_LABELS: Record<string, string> = {
  Land: "Grass",
  Land1: "Grass",
  Land2: "Grass",
  Land3: "Grass",
  LandDay: "Grass (Day)",
  LandMorning: "Grass (Morning)",
  LandNight: "Grass (Night)",
  LandFog: "Grass (Fog)",
  LandRain: "Grass (Rain)",
  LandStorm: "Grass (Storm)",
  LandSunny: "Grass (Sun)",
  LandWind: "Grass (Wind)",
  TallGrass: "Tall Grass",
  Cave: "Cave",
  Water: "Surf",
  WaterNight: "Surf (Night)",
  WaterFog: "Surf (Fog)",
  WaterRain: "Surf (Rain)",
  WaterStorm: "Surf (Storm)",
  WaterSunny: "Surf (Sun)",
  WaterWind: "Surf (Wind)",
  OldRod: "Old Rod",
  GoodRod: "Good Rod",
  SuperRod: "Super Rod",
  RockSmash: "Rock Smash",
  Static: "Static",
  Gift: "Gift",
  Roaming: "Roaming",
  PokeRadar: "Poké Radar",
};
export function methodLabel(method: string): string {
  return METHOD_LABELS[method] ?? method;
}

const LEVEL_NOTE_SUFFIX: Record<string, string> = {
  none: "",
  day: " (Day)",
  night: " (Night)",
  atk_gt_def: " (Atk > Def)",
  atk_lt_def: " (Atk < Def)",
  atk_eq_def: " (Atk = Def)",
};
// compose an evolution condition into its display sentence
export function formatCondition(c: EvoCondition): string {
  switch (c.kind) {
    case "level":
      return `Lv. ${c.level}${LEVEL_NOTE_SUFFIX[c.note] ?? ""}`;
    case "use_item":
      return `Use ${c.item}`;
    case "hold_day":
      return `Hold ${c.item} (Day)`;
    case "trade":
      return `Trade holding ${c.item}`;
    case "know_move":
      return `Level up knowing ${c.name}`;
  }
}

// the learn-method chips for a move (e.g. ["Lv. 7", "TM32", "Egg"])
export function learnLabels(s: LearnSources): string[] {
  const out: string[] = [];
  if (s.level !== null) out.push(s.level === 0 ? "Evolve" : `Lv. ${s.level}`);
  if (s.machine !== null) out.push(s.machine);
  if (s.tutor) out.push("Tutor");
  if (s.egg) out.push("Egg");
  if (s.expert !== null)
    out.push(s.expert ? "Legendary Move Expert" : "Move Expert");
  return out;
}
