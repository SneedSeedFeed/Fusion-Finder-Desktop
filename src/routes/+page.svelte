<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import FusionInspector from "$lib/FusionInspector.svelte";
  import AreaPanel from "$lib/AreaPanel.svelte";
  import MoveHoverHost from "$lib/MoveHoverHost.svelte";
  import Setup from "$lib/Setup.svelte";
  import ResultsToolbar from "$lib/search/ResultsToolbar.svelte";
  import ResultsGrid from "$lib/search/ResultsGrid.svelte";
  import FilterSidebar from "$lib/search/FilterSidebar.svelte";
  import { FilterState } from "$lib/searchFilters.svelte";
  import { Favourites } from "$lib/favourites.svelte";
  import {
    SYNERGY_STATS,
    type Bootstrap,
    type GameConfig,
    type Metric,
    type SynergyStat,
  } from "$lib/bindings";

  // undefined = still checking which game is loaded, null = no game (show setup), else loaded
  let config = $state<GameConfig | null | undefined>(undefined);
  // true while the user is (re-)running setup from inside the app via "change game"
  let changingGame = $state(false);
  let options = $state<Bootstrap | null>(null);
  let error = $state<string | null>(null);
  let results = $state<Uint32Array>(new Uint32Array());
  let searching = $state(false);
  // the fusion (head/body species indices) shown in the inspect modal, if any
  let inspecting = $state<{ head: number; body: number } | null>(null);
  // whether the (non-blocking) area/route encounter panel is open
  let showAreas = $state(false);
  // null metric = dex (order we load the data in) metric2 switches to ratio sorting.
  let metric = $state<Metric | null>(null);
  let metric2 = $state<Metric | null>(null);
  let sortDesc = $state(false);
  // which stats count toward the synergy metrics (all by default); only used by Synergy sorts
  let synergyStats = $state<SynergyStat[]>(SYNERGY_STATS.map((s) => s.value));

  // when on, the grid shows the user's favourites instead of the search results
  let showFavourites = $state(false);

  // dex id -> our species index for the loaded game (favourites are stored by stable dex id, but
  // the grid/inspector work in species indices)
  let dexToIndex = $derived.by(() => {
    const m = new Map<number, number>();
    if (options) for (const s of options.species) m.set(s.dex_id, s.id);
    return m;
  });

  // favourited fusions that exist in the loaded game, encoded as grid ids (`head * count + body`).
  // Favourites from another game version whose species aren't present here are skipped.
  let favouriteIds = $derived.by(() => {
    if (!options) return new Uint32Array();
    const n = options.species_count;
    const ids: number[] = [];
    for (const f of favourites.entries) {
      const head = dexToIndex.get(f.head_dex);
      const body = dexToIndex.get(f.body_dex);
      if (head != null && body != null) ids.push(head * n + body);
    }
    return new Uint32Array(ids);
  });

  // backend always returns the canonical ascending order sort direction is pure presentation, so descending is just this list read in reverse
  // saves us re-computing the set just for flipping from asc to desc
  let displayed = $derived(
    showFavourites ? favouriteIds : sortDesc ? results.toReversed() : results,
  );

  // $state so it can be passed with `bind:` to FilterSidebar (which binds its sub-properties into
  // child components); the instance is never reassigned, only its reactive fields mutate.
  let filters = $state(new FilterState());

  // starred fusions, keyed by stable national-dex ids; loaded once and shared with the grid +
  // inspector. Independent of the loaded game.
  let favourites = $state(new Favourites());

  async function runSearch(
    payload: Record<string, unknown>,
    sortMetric: Metric | null,
    ratioMetric: Metric | null,
    synergy: SynergyStat[],
  ) {
    searching = true;
    try {
      const buf = await invoke<ArrayBuffer>("search", {
        filters: payload,
        metric: sortMetric,
        metric2: ratioMetric,
        synergy,
      });
      results = new Uint32Array(buf);
    } catch (e) {
      error = String(e);
    } finally {
      searching = false;
    }
  }

  // Load the dex options for the currently loaded game and seed the filters (slider bounds, id cap).
  async function loadDex() {
    error = null;
    try {
      options = await invoke<Bootstrap>("bootstrap");
      filters.reset(options);
    } catch (e) {
      error = String(e);
    }
  }

  // Called by the Setup splash once a game has loaded (first run or "change game").
  function onGameReady(c: GameConfig) {
    config = c;
    changingGame = false;
    options = null;
    results = new Uint32Array();
    loadDex();
  }

  onMount(async () => {
    favourites.load();
    config = await invoke<GameConfig | null>("current_game");
    if (config) loadDex();
  });

  // live search: any filter/metric change schedules a (debounced) search, cancelling the previous one
  $effect(() => {
    if (!options) return;
    const payload = filters.build(options); // reads filter state -> registers deps
    const sortMetric = metric;
    const ratioMetric = metric2;
    const synergy = [...synergyStats]; // register dep; snapshot for the debounced call
    const handle = setTimeout(
      () => runSearch(payload, sortMetric, ratioMetric, synergy),
      200,
    );
    return () => clearTimeout(handle);
  });
</script>

{#if config === undefined}
  <main
    class="flex h-screen items-center justify-center bg-[#0d1117] text-sm text-gray-400"
  >
    Loading…
  </main>
{:else if config === null || changingGame}
  <Setup
    onReady={onGameReady}
    onCancel={changingGame ? () => (changingGame = false) : undefined}
  />
{:else}
  <main
    class="flex h-screen overflow-hidden bg-[#0d1117] text-sm text-gray-200"
  >
    {#if error}
      <p class="p-4 text-red-400">Couldn't load game data: {error}</p>
    {:else if !options}
      <p class="p-4 text-gray-400">Loading game data…</p>
    {:else}
      <section class="flex min-w-0 flex-1 flex-col">
        <ResultsToolbar
          count={displayed.length}
          {searching}
          bind:metric
          bind:metric2
          bind:sortDesc
          bind:synergyStats
          bind:showFavourites
          favouritesCount={favourites.size}
          version={config.version}
          onChangeGame={() => (changingGame = true)}
          onOpenAreas={() => (showAreas = true)}
        />
        {#if showFavourites && displayed.length === 0}
          <p class="p-6 text-center text-gray-400">No favourites yet</p>
        {:else}
          <ResultsGrid
            {options}
            {favourites}
            results={displayed}
            onInspect={(f) => (inspecting = f)}
          />
        {/if}
      </section>

      <FilterSidebar bind:filters {options} />

      {#if showAreas}
        <AreaPanel
          types={options.types}
          onClose={() => (showAreas = false)}
          onInspect={(f) => (inspecting = f)}
        />
      {/if}

      {#if inspecting}
        <FusionInspector
          head={inspecting.head}
          body={inspecting.body}
          types={options.types}
          {favourites}
          onClose={() => (inspecting = null)}
        />
      {/if}

      <!-- singleton hover-card for moves in the filter list and inspector -->
      <MoveHoverHost types={options.types} />
    {/if}
  </main>
{/if}
