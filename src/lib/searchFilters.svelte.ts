import type { Bootstrap, EvolutionFilter, Range, StatKey } from "$lib/bindings";
import { STATS } from "$lib/bindings";

// All filter inputs for the search, plus the small mutators and the conversions to/from the
// backend. Held as a single reactive object so the page and the filter components share one
// source of truth instead of threading ~20 pieces of state around.
export class FilterState {
  hasPokemon = $state<number | null>(null);
  pokemonPosition = $state<"Either" | "Head" | "Body">("Either");
  selectedTypes = $state<number[]>([]);
  monoType = $state(false);
  defenseRelation = $state<"Weak" | "Resist" | "Immune">("Resist");
  defenseTypes = $state<number[]>([]);
  abilityId = $state<number | null>(null);
  abilitySlot = $state<"Normal" | "Hidden" | "Either">("Either");
  moveIds = $state<number[]>([]);
  moveEgg = $state(true);
  moveLevel = $state(true);
  moveTutor = $state(true);
  // move-list filters (client-side; the move pool is small)
  moveSearch = $state("");
  moveTypeFilter = $state<number | null>(null);
  moveCategoryFilter = $state<0 | 1 | 2 | null>(null);
  movePowerMin = $state<number | null>(null);
  moveFlagFilter = $state<string[]>([]);
  hasCustomSprite = $state(false);
  excludeLegendaries = $state(false);
  evolution = $state<EvolutionFilter | null>(null);
  // hidden, game-set: caps fusable species to the real dex (Kanto's data has unfusable Gen-3 mons)
  blockIdsAbove = $state<number | null>(null);
  // per-stat slider position as [min, max]; a stat only constrains the search when its range is
  // narrower than the full bounds (otherwise we send None for it).
  statRange = $state<Record<StatKey, [number, number]>>(
    {} as Record<StatKey, [number, number]>,
  );

  toggleMoveFlag(f: string) {
    this.moveFlagFilter = this.moveFlagFilter.includes(f)
      ? this.moveFlagFilter.filter((x) => x !== f)
      : [...this.moveFlagFilter, f];
  }

  addMove(id: number) {
    if (!this.moveIds.includes(id)) this.moveIds = [...this.moveIds, id];
  }

  removeMove(id: number) {
    this.moveIds = this.moveIds.filter((m) => m !== id);
  }

  // Reset every filter to its default and seed the per-game bits (stat sliders, id cap) from the
  // freshly-loaded dex. Called on game load and by the "Clear all" button.
  reset(options: Bootstrap) {
    this.hasPokemon = null;
    this.pokemonPosition = "Either";
    this.selectedTypes = [];
    this.monoType = false;
    this.defenseRelation = "Resist";
    this.defenseTypes = [];
    this.abilityId = null;
    this.abilitySlot = "Either";
    this.moveIds = [];
    this.moveSearch = "";
    this.moveTypeFilter = null;
    this.moveCategoryFilter = null;
    this.movePowerMin = null;
    this.moveFlagFilter = [];
    this.moveLevel = true;
    this.moveTutor = true;
    this.moveEgg = true;
    this.hasCustomSprite = false;
    this.excludeLegendaries = false;
    this.evolution = null;
    this.blockIdsAbove = options.block_ids_above;
    this.statRange = Object.fromEntries(
      STATS.map((s) => [
        s,
        [options.stat_bounds[s].min, options.stat_bounds[s].max],
      ]),
    ) as Record<StatKey, [number, number]>;
  }

  // The payload the backend `search` command expects — only includes a filter when it's active.
  build(options: Bootstrap): Record<string, unknown> {
    const filters: Record<string, unknown> = {};
    if (this.hasPokemon !== null)
      filters.has_pokemon = { [this.pokemonPosition]: this.hasPokemon };
    if (this.selectedTypes.length) filters.has_type = this.selectedTypes;
    if (this.monoType) filters.mono_type = true;
    if (this.defenseTypes.length)
      filters.defense = {
        relation: this.defenseRelation,
        types: this.defenseTypes,
      };
    if (this.abilityId !== null)
      filters.has_ability = { [this.abilitySlot]: this.abilityId };
    if (this.moveIds.length) {
      filters.has_move = {
        egg: this.moveEgg,
        level: this.moveLevel,
        tutor: this.moveTutor,
        moves: this.moveIds,
      };
    }
    if (this.hasCustomSprite) filters.has_custom_sprite = true;
    if (this.excludeLegendaries) filters.exclude_legendaries = true;
    if (this.evolution !== null) filters.evolution = this.evolution;
    if (this.blockIdsAbove !== null)
      filters.block_ids_above = this.blockIdsAbove;
    // a stat is active only if its slider has moved off the full bounds; send the whole object
    // (active stats as {min,max}, the rest as null) so the backend leaves them open.
    const sr: Record<string, Range | null> = {};
    let any = false;
    for (const s of STATS) {
      const b = options.stat_bounds[s];
      const [lo, hi] = this.statRange[s];
      const active = lo > b.min || hi < b.max;
      sr[s] = active ? { min: lo, max: hi } : null;
      any ||= active;
    }
    if (any) filters.stat_range = sr;
    return filters;
  }
}
