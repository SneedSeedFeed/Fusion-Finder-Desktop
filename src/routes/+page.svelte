<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import FusionInspector from "$lib/FusionInspector.svelte";
  import AreaPanel from "$lib/AreaPanel.svelte";
  import Setup from "$lib/Setup.svelte";
  import ResultsToolbar from "$lib/search/ResultsToolbar.svelte";
  import ResultsGrid from "$lib/search/ResultsGrid.svelte";
  import FilterSidebar from "$lib/search/FilterSidebar.svelte";
  import { FilterState } from "$lib/searchFilters.svelte";
  import type { Bootstrap, GameConfig, SortBy } from "$lib/bindings";

  // undefined = still checking which game is loaded, null = no game (show setup), else loaded
  let config = $state<GameConfig | null | undefined>(undefined);
  // true while the user is (re-)running setup from inside the app via "change game"
  let changingGame = $state(false);
  let options = $state<Bootstrap | null>(null);
  let error = $state<string | null>(null);
  let results = $state<number[]>([]);
  let searching = $state(false);
  // the fusion (head/body species indices) shown in the inspect modal, if any
  let inspecting = $state<{ head: number; body: number } | null>(null);
  // whether the (non-blocking) area/route encounter panel is open
  let showAreas = $state(false);
  let sortBy = $state<SortBy>("DexNumber");
  let sortDesc = $state(false);

  // $state so it can be passed with `bind:` to FilterSidebar (which binds its sub-properties into
  // child components); the instance is never reassigned, only its reactive fields mutate.
  let filters = $state(new FilterState());

  async function runSearch(
    payload: Record<string, unknown>,
    sort: SortBy,
    descending: boolean,
  ) {
    searching = true;
    try {
      results = await invoke<number[]>("search", {
        filters: payload,
        sort,
        descending,
      });
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
    results = [];
    loadDex();
  }

  onMount(async () => {
    config = await invoke<GameConfig | null>("current_game");
    if (config) loadDex();
  });

  // live search: any filter/sort change schedules a (debounced) search, cancelling the previous one.
  $effect(() => {
    if (!options) return;
    const payload = filters.build(options); // reads filter state -> registers deps
    const sort = sortBy;
    const desc = sortDesc;
    const handle = setTimeout(() => runSearch(payload, sort, desc), 200);
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
  <main class="flex h-screen overflow-hidden bg-[#0d1117] text-sm text-gray-200">
    {#if error}
      <p class="p-4 text-red-400">Couldn't load game data: {error}</p>
    {:else if !options}
      <p class="p-4 text-gray-400">Loading game data…</p>
    {:else}
      <section class="flex min-w-0 flex-1 flex-col">
        <ResultsToolbar
          count={results.length}
          {searching}
          bind:sortBy
          bind:sortDesc
          version={config.version}
          onChangeGame={() => (changingGame = true)}
          onOpenAreas={() => (showAreas = true)}
        />
        <ResultsGrid {options} {results} onInspect={(f) => (inspecting = f)} />
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
          onClose={() => (inspecting = null)}
        />
      {/if}
    {/if}
  </main>
{/if}
