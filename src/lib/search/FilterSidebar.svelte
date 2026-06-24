<script lang="ts">
  import type { Bootstrap } from "$lib/bindings";
  import type { FilterState } from "$lib/searchFilters.svelte";
  import Combobox from "$lib/Combobox.svelte";
  import TypePicker from "$lib/search/TypePicker.svelte";
  import DefenseMatchupPicker from "$lib/search/DefenseMatchupPicker.svelte";
  import StatSliders from "$lib/search/StatSliders.svelte";
  import MovePicker from "$lib/search/MovePicker.svelte";

  // `filters` is bindable so the ownership chain is explicit: this component binds sub-properties
  // of it (filters.selectedTypes, filters.statRange, …) into child components, which Svelte only
  // allows when the prop itself was received via `bind:`.
  let {
    filters = $bindable(),
    options,
  }: { filters: FilterState; options: Bootstrap } = $props();

  // species available to pick — honours the hidden id cap so capped (e.g. Gen-3) mons aren't offered
  const pickableSpecies = $derived(
    filters.blockIdsAbove === null
      ? options.species
      : options.species.filter((s) => s.dex_id <= filters.blockIdsAbove!),
  );

  const speciesName = $derived(
    new Map(options.species.map((s) => [s.id, s.name])),
  );

  // the ignore-list combobox is "add then clear": picking a species folds it into the list and
  // resets the box for the next pick.
  let ignorePick = $state<number | null>(null);
  $effect(() => {
    if (ignorePick !== null) {
      filters.addIgnored(ignorePick);
      ignorePick = null;
    }
  });
</script>

<aside
  class="w-80 shrink-0 overflow-auto border-l border-gray-800 bg-gray-900/40 p-3"
>
  <div class="mb-2 flex items-center justify-between">
    <h2 class="text-base font-semibold text-gray-100">Filters</h2>
    <button
      type="button"
      class="rounded border border-gray-700 bg-gray-800 px-2 py-0.5 text-xs text-gray-300 hover:bg-gray-700"
      onclick={() => filters.reset(options)}>Clear all</button
    >
  </div>

  <label class="mb-3 flex items-center gap-2 text-sm">
    <input
      type="checkbox"
      class="accent-blue-500"
      bind:checked={filters.excludeLegendaries}
    />
    Exclude legendaries
  </label>

  <label class="mb-3 flex items-center gap-2 text-sm">
    Custom sprite
    <select
      class="ml-auto rounded border border-gray-700 bg-gray-800 p-1 text-xs text-gray-200"
      bind:value={filters.customSprite}
    >
      <option value={null}>Any</option>
      <option value="Custom">Only custom sprites</option>
      <option value="Autogen">Only autogen sprites</option>
    </select>
  </label>

  <label class="mb-3 flex items-center gap-2 text-sm">
    Evolution
    <select
      class="ml-auto rounded border border-gray-700 bg-gray-800 p-1 text-xs text-gray-200"
      bind:value={filters.evolution}
    >
      <option value={null}>Any</option>
      <option value="CanEvolve">Can still evolve</option>
      <option value="FullyEvolved">Fully evolved</option>
    </select>
  </label>

  <fieldset class="mb-3 rounded-md border border-gray-800 p-2">
    <legend class="px-1 text-sm font-semibold text-gray-300">Types</legend>
    <TypePicker
      types={options.types}
      bind:value={filters.selectedTypes}
      max={2}
    />
    <label class="mt-2 flex items-center gap-2 text-sm">
      <input
        type="checkbox"
        class="accent-blue-500"
        bind:checked={filters.monoType}
      />
      Mono-type only
    </label>
  </fieldset>

  <fieldset class="mb-3 rounded-md border border-gray-800 p-2">
    <legend class="px-1 text-sm font-semibold text-gray-300">Defense</legend>
    <DefenseMatchupPicker
      types={options.types}
      bind:value={filters.defenseMatchups}
    />
  </fieldset>

  <fieldset class="mb-3 rounded-md border border-gray-800 p-2">
    <legend class="px-1 text-sm font-semibold text-gray-300">Stats</legend>
    <StatSliders bounds={options.stat_bounds} bind:value={filters.statRange} />
  </fieldset>

  <fieldset class="mb-3 rounded-md border border-gray-800 p-2">
    <legend class="px-1 text-sm font-semibold text-gray-300"
      >Contains Pokémon</legend
    >
    <div class="mb-1">
      <Combobox items={pickableSpecies} bind:value={filters.hasPokemon} />
    </div>
    <select
      class="w-full rounded border border-gray-700 bg-gray-800 p-1 text-gray-200 disabled:opacity-50"
      bind:value={filters.pokemonPosition}
      disabled={filters.hasPokemon === null}
    >
      <option value="Either">either side</option>
      <option value="Head">as head</option>
      <option value="Body">as body</option>
    </select>
  </fieldset>

  <fieldset class="mb-3 rounded-md border border-gray-800 p-2">
    <legend class="px-1 text-sm font-semibold text-gray-300">Ability</legend>
    <div class="mb-1">
      <Combobox items={options.abilities} bind:value={filters.abilityId} />
    </div>
    <select
      class="w-full rounded border border-gray-700 bg-gray-800 p-1 text-gray-200 disabled:opacity-50"
      bind:value={filters.abilitySlot}
      disabled={filters.abilityId === null}
    >
      <option value="Either">either slot</option>
      <option value="Normal">normal</option>
      <option value="Hidden">hidden</option>
    </select>
  </fieldset>

  <fieldset class="mb-3 rounded-md border border-gray-800 p-2">
    <legend class="px-1 text-sm font-semibold text-gray-300"
      >Ignore Pokémon</legend
    >
    <div class="mb-1">
      <Combobox
        items={pickableSpecies}
        bind:value={ignorePick}
        placeholder="-- add to block list --"
      />
    </div>
    {#if filters.ignoredSpecies.length}
      <div class="flex flex-wrap gap-1">
        {#each filters.ignoredSpecies as id (id)}
          <button
            type="button"
            class="flex items-center gap-1 rounded border border-gray-700 bg-gray-800 px-1.5 py-0.5 text-xs text-gray-200 hover:border-red-500"
            title="Remove from block list"
            onclick={() => filters.removeIgnored(id)}
          >
            {speciesName.get(id) ?? `#${id}`}
            <span class="text-gray-400">×</span>
          </button>
        {/each}
      </div>
    {/if}
  </fieldset>

  <MovePicker bind:filters moves={options.moves} types={options.types} />
</aside>
