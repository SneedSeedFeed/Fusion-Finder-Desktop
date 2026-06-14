<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import RangeSlider from "$lib/RangeSlider.svelte";
  import Combobox from "$lib/Combobox.svelte";

  interface NamedId { id: number; name: string }
  interface SpeciesOption { id: number; name: string; first: string; second: string }
  interface Range { min: number; max: number }
  interface StatBounds {
    hp: Range; atk: Range; def: Range; spa: Range; spd: Range; spe: Range; bst: Range;
  }
  interface FilterOptions {
    species_count: number;
    species: SpeciesOption[];
    moves: NamedId[];
    types: NamedId[];
    abilities: NamedId[];
    stat_bounds: StatBounds;
  }

  const STATS = ["hp", "atk", "def", "spa", "spd", "spe", "bst"] as const;
  type StatKey = (typeof STATS)[number];

  let options = $state<FilterOptions | null>(null);
  let error = $state<string | null>(null);
  let results = $state<number[]>([]);
  let searching = $state(false);

  // --- filter inputs ---
  let hasPokemon = $state<number | null>(null);
  let selectedTypes = $state<number[]>([]);
  let abilityId = $state<number | null>(null);
  let abilitySlot = $state<"Normal" | "Hidden" | "Either">("Either");
  let moveIds = $state<number[]>([]);
  let moveEgg = $state(false);
  let moveLevel = $state(true);
  let moveTutor = $state(false);
  // per-stat slider position as [min, max]; a stat only constrains the search when its
  // range is narrower than the full bounds (otherwise we send None for it).
  let statRange = $state<Record<StatKey, [number, number]>>({} as Record<StatKey, [number, number]>);

  function speciesName(id: number): string {
    return options?.species[id]?.name ?? `#${id}`;
  }

  // a fusion's name is the head's first half + the body's second half
  function fusionName(head: number, body: number): string {
    const h = options?.species[head];
    const b = options?.species[body];
    return h && b ? h.first + b.second : "";
  }

  function decode(id: number): { head: number; body: number } {
    const n = options!.species_count;
    return { head: Math.floor(id / n), body: id % n };
  }

  function toggleType(id: number) {
    if (selectedTypes.includes(id)) {
      selectedTypes = selectedTypes.filter((t) => t !== id);
    } else if (selectedTypes.length < 2) {
      selectedTypes = [...selectedTypes, id];
    }
  }

  function buildFilters() {
    const filters: Record<string, unknown> = {};
    if (hasPokemon !== null) filters.has_pokemon = hasPokemon;
    if (selectedTypes.length) filters.has_type = selectedTypes;
    if (abilityId !== null) filters.has_ability = { [abilitySlot]: abilityId };
    if (moveIds.length) {
      filters.has_move = { egg: moveEgg, level: moveLevel, tutor: moveTutor, moves: moveIds };
    }
    if (options) {
      // a stat is active only if its slider has moved off the full bounds; send the whole
      // object (active stats as {min,max}, the rest as null) so the backend leaves them open.
      const sr: Record<string, Range | null> = {};
      let any = false;
      for (const s of STATS) {
        const b = options.stat_bounds[s];
        const [lo, hi] = statRange[s];
        const active = lo > b.min || hi < b.max;
        sr[s] = active ? { min: lo, max: hi } : null;
        any ||= active;
      }
      if (any) filters.stat_range = sr;
    }
    return filters;
  }

  async function runSearch(filters: Record<string, unknown>) {
    searching = true;
    try {
      results = await invoke<number[]>("search", { filters });
      scrollTop = 0;
      scroller?.scrollTo({ top: 0 });
    } catch (e) {
      error = String(e);
    } finally {
      searching = false;
    }
  }

  onMount(async () => {
    try {
      options = await invoke<FilterOptions>("bootstrap");
      for (const s of STATS) {
        statRange[s] = [options.stat_bounds[s].min, options.stat_bounds[s].max];
      }
    } catch (e) {
      error = String(e);
    }
  });

  // live search: any filter change schedules a (debounced) search, cancelling the previous one.
  $effect(() => {
    if (!options) return;
    const filters = buildFilters(); // reads filter state -> registers deps
    const handle = setTimeout(() => runSearch(filters), 200);
    return () => clearTimeout(handle);
  });

  // --- virtual grid ---
  const COL_W = 120; // target card width; columns auto-fit the pane width
  const ROW_H = 150; // card height (96 sprite + two text lines)
  const GAP = 8;
  const OVERSCAN = 4; // extra rows above/below the viewport to keep scrolling smooth

  let scroller = $state<HTMLDivElement | null>(null);
  let scrollTop = $state(0);
  let viewportW = $state(0);
  let viewportH = $state(0);

  const cols = $derived(Math.max(1, Math.floor((viewportW + GAP) / (COL_W + GAP))));
  const rowStride = ROW_H + GAP;
  const totalRows = $derived(Math.ceil(results.length / cols));
  const totalHeight = $derived(totalRows * rowStride);
  const firstRow = $derived(Math.max(0, Math.floor(scrollTop / rowStride) - OVERSCAN));
  const visibleRows = $derived(Math.ceil(viewportH / rowStride) + 2 * OVERSCAN);
  const start = $derived(firstRow * cols);
  const end = $derived(Math.min(results.length, (firstRow + visibleRows) * cols));
  const offsetY = $derived(firstRow * rowStride);
  const visible = $derived(results.slice(start, end));
</script>

<main class="flex h-screen overflow-hidden text-sm text-gray-800">
  {#if error}
    <p class="p-4 text-red-600">Couldn't load game data: {error}</p>
  {:else if !options}
    <p class="p-4 text-gray-500">Loading game data…</p>
  {:else}
    <section class="flex min-w-0 flex-1 flex-col">
      <header class="flex items-center gap-2 border-b border-gray-200 px-3 py-2">
        <strong>{results.length.toLocaleString()}</strong> fusions
        {#if searching}<span class="text-gray-400">· searching…</span>{/if}
      </header>

      <div
        bind:this={scroller}
        bind:clientWidth={viewportW}
        bind:clientHeight={viewportH}
        onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
        class="flex-1 overflow-auto p-2"
      >
        <div class="relative w-full" style="height: {totalHeight}px">
          <div
            class="absolute inset-x-0 grid"
            style="top: {offsetY}px; gap: {GAP}px; grid-template-columns: repeat({cols}, minmax(0, 1fr));"
          >
            {#each visible as id (id)}
              {@const f = decode(id)}
              <div
                class="flex flex-col items-center overflow-hidden rounded border border-gray-200 p-1.5 text-center leading-tight"
                style="height: {ROW_H}px"
                title={`${speciesName(f.head)} / ${speciesName(f.body)}`}
              >
                <div class="mb-1 size-24 shrink-0 rounded border border-dashed border-gray-300 bg-gray-100" aria-hidden="true"></div>
                <span class="w-full truncate font-semibold">{fusionName(f.head, f.body)}</span>
                <span class="w-full truncate text-[0.65rem] text-gray-500">{speciesName(f.head)} / {speciesName(f.body)}</span>
              </div>
            {/each}
          </div>
        </div>
      </div>
    </section>

    <aside class="w-80 shrink-0 overflow-auto border-l border-gray-200 p-3">
      <h2 class="mb-2 text-base font-semibold">Filters</h2>

      <fieldset class="mb-3 rounded-md border border-gray-200 p-2">
        <legend class="px-1 text-sm font-semibold">Types <span class="font-normal text-gray-400">(up to 2)</span></legend>
        <div class="flex flex-wrap gap-1">
          {#each options.types as t (t.id)}
            <button
              type="button"
              class="rounded-full border px-2 py-0.5 text-xs {selectedTypes.includes(t.id)
                ? 'border-blue-600 bg-blue-600 text-white'
                : 'border-gray-300 bg-white'}"
              onclick={() => toggleType(t.id)}>{t.name}</button>
          {/each}
        </div>
      </fieldset>

      <fieldset class="mb-3 rounded-md border border-gray-200 p-2">
        <legend class="px-1 text-sm font-semibold">Stats</legend>
        {#each STATS as s (s)}
          {@const b = options.stat_bounds[s]}
          {@const active = statRange[s][0] > b.min || statRange[s][1] < b.max}
          <div class="mb-3">
            <div class="mb-1 flex items-center justify-between text-xs">
              <span class="font-medium {active ? 'text-blue-700' : 'text-gray-500'}">{s.toUpperCase()}</span>
              <span class="tabular-nums text-gray-500">{statRange[s][0]} – {statRange[s][1]}</span>
            </div>
            <RangeSlider min={b.min} max={b.max} bind:value={statRange[s]} />
          </div>
        {/each}
      </fieldset>

      <fieldset class="mb-3 rounded-md border border-gray-200 p-2">
        <legend class="px-1 text-sm font-semibold">Contains Pokémon</legend>
        <Combobox items={options.species} bind:value={hasPokemon} />
      </fieldset>

      <fieldset class="mb-3 rounded-md border border-gray-200 p-2">
        <legend class="px-1 text-sm font-semibold">Ability</legend>
        <div class="mb-1"><Combobox items={options.abilities} bind:value={abilityId} /></div>
        <select class="w-full rounded border border-gray-300 p-1 disabled:opacity-50" bind:value={abilitySlot} disabled={abilityId === null}>
          <option value="Either">either slot</option>
          <option value="Normal">normal</option>
          <option value="Hidden">hidden</option>
        </select>
      </fieldset>

      <fieldset class="mb-3 rounded-md border border-gray-200 p-2">
        <legend class="px-1 text-sm font-semibold">Moves <span class="font-normal text-gray-400">(learns all)</span></legend>
        <select class="w-full rounded border border-gray-300 p-1" multiple size="6" bind:value={moveIds}>
          {#each options.moves as m (m.id)}<option value={m.id}>{m.name}</option>{/each}
        </select>
        <div class="mt-1 flex gap-3 text-xs">
          <label class="flex items-center gap-1"><input type="checkbox" bind:checked={moveLevel} /> level</label>
          <label class="flex items-center gap-1"><input type="checkbox" bind:checked={moveTutor} /> tutor</label>
          <label class="flex items-center gap-1"><input type="checkbox" bind:checked={moveEgg} /> egg</label>
        </div>
      </fieldset>
    </aside>
  {/if}
</main>
