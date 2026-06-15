<script lang="ts">
  import type { MoveOption, NamedId } from "$lib/bindings";
  import { MOVE_CATEGORY, MOVE_FLAGS } from "$lib/bindings";
  import { typeIcon, categoryIcon, typeNameMap } from "$lib/typeIcon";
  import type { FilterState } from "$lib/searchFilters.svelte";

  // The "Moves" filter: selected-move chips, a client-side filterable list over the (small) move
  // pool, and the head/body learn-source toggles. Owns its own list deriveds; the chosen move ids
  // and list-filter inputs live on the shared `filters`.
  let {
    filters,
    moves,
    types,
  }: { filters: FilterState; moves: MoveOption[]; types: NamedId[] } = $props();

  const MOVE_LIST_CAP = 200;

  const typeNames = $derived(typeNameMap(types));
  function typeName(id: number): string {
    return typeNames.get(id) ?? "";
  }
  function titleCase(s: string): string {
    return s.charAt(0) + s.slice(1).toLowerCase();
  }

  const moveById = $derived(new Map(moves.map((m) => [m.id, m])));
  // distinct move types as {id, name}, ordered by name, for the move-type dropdown
  const moveTypes = $derived(
    [...new Set(moves.map((m) => m.ty))]
      .map((id) => ({ id, name: typeName(id) }))
      .sort((a, b) => a.name.localeCompare(b.name)),
  );
  const selectedMoves = $derived(
    filters.moveIds
      .map((id) => moveById.get(id))
      .filter((m): m is MoveOption => !!m),
  );
  const filteredMoves = $derived.by(() => {
    const q = filters.moveSearch.trim().toLowerCase();
    const chosen = new Set(filters.moveIds);
    return moves
      .filter(
        (m) =>
          !chosen.has(m.id) &&
          (filters.moveTypeFilter === null ||
            m.ty === filters.moveTypeFilter) &&
          (filters.moveCategoryFilter === null ||
            m.category === filters.moveCategoryFilter) &&
          (filters.movePowerMin == null ||
            (m.power ?? 0) >= filters.movePowerMin) &&
          filters.moveFlagFilter.every((f) => m.flags.includes(f)) &&
          (!q ||
            m.name.toLowerCase().includes(q) ||
            m.description.toLowerCase().includes(q)),
      )
      .slice(0, MOVE_LIST_CAP);
  });
</script>

<fieldset class="mb-3 rounded-md border border-gray-800 p-2">
  <legend class="px-1 text-sm font-semibold text-gray-300"
    >Moves <span class="font-normal text-gray-500">(learns all)</span></legend
  >

  <!-- selected moves as removable chips -->
  {#if selectedMoves.length}
    <div class="mb-2 flex flex-wrap gap-1">
      {#each selectedMoves as m (m.id)}
        <button
          type="button"
          class="flex items-center gap-1 rounded-full bg-blue-600 py-0.5 pr-1 pl-2 text-xs text-white hover:bg-blue-500"
          title="Remove {m.name}"
          onclick={() => filters.removeMove(m.id)}
        >
          {m.name}<span class="text-blue-200">×</span>
        </button>
      {/each}
    </div>
  {/if}

  <!-- filters over the move pool -->
  <input
    type="search"
    placeholder="search name or effect…"
    bind:value={filters.moveSearch}
    class="mb-1 w-full rounded border border-gray-700 bg-gray-800 p-1 text-gray-200 placeholder:text-gray-500"
  />
  <div class="mb-1 flex gap-1">
    <select
      class="min-w-0 flex-1 rounded border border-gray-700 bg-gray-800 p-1 text-xs text-gray-200"
      bind:value={filters.moveTypeFilter}
    >
      <option value={null}>any type</option>
      {#each moveTypes as t (t.id)}<option value={t.id}
          >{titleCase(t.name)}</option
        >{/each}
    </select>
    <select
      class="min-w-0 flex-1 rounded border border-gray-700 bg-gray-800 p-1 text-xs text-gray-200"
      bind:value={filters.moveCategoryFilter}
    >
      <option value={null}>any category</option>
      <option value={0}>Physical</option>
      <option value={1}>Special</option>
      <option value={2}>Status</option>
    </select>
    <input
      type="number"
      min="0"
      placeholder="pow"
      bind:value={filters.movePowerMin}
      class="w-16 rounded border border-gray-700 bg-gray-800 p-1 text-xs text-gray-200 placeholder:text-gray-500"
    />
  </div>
  <div class="mb-1 flex flex-wrap gap-1">
    {#each MOVE_FLAGS as [display, flag] (flag)}
      <button
        type="button"
        class="rounded-full border px-2 py-0.5 text-xs {filters.moveFlagFilter.includes(
          flag,
        )
          ? 'border-blue-500 bg-blue-600 text-white'
          : 'border-gray-700 bg-gray-800 text-gray-400 hover:border-gray-600'}"
        aria-pressed={filters.moveFlagFilter.includes(flag)}
        onclick={() => filters.toggleMoveFlag(flag)}>{display}</button
      >
    {/each}
  </div>

  <!-- filtered, clickable list -->
  <ul class="max-h-48 overflow-auto rounded border border-gray-800">
    {#each filteredMoves as m (m.id)}
      <li>
        <button
          type="button"
          class="flex w-full items-center gap-2 px-2 py-1 text-left text-sm hover:bg-gray-800"
          title={m.description}
          onclick={() => filters.addMove(m.id)}
        >
          <span class="flex-1 truncate">{m.name}</span>
          <img
            src={typeIcon(typeName(m.ty))}
            alt={typeName(m.ty)}
            class="h-3.5"
          />
          <span class="inline-flex rounded bg-gray-700 p-0.5"
            ><img
              src={categoryIcon(m.category)}
              alt={MOVE_CATEGORY[m.category]}
              class="h-3"
            /></span
          >
          <span class="w-7 text-right text-xs tabular-nums text-gray-400"
            >{m.power ?? "—"}</span
          >
        </button>
      </li>
    {:else}
      <li class="px-2 py-2 text-xs text-gray-500">no matching moves</li>
    {/each}
  </ul>

  <div class="mt-1 flex gap-3 text-xs">
    <label class="flex items-center gap-1"
      ><input
        type="checkbox"
        class="accent-blue-500"
        bind:checked={filters.moveLevel}
      /> level</label
    >
    <label class="flex items-center gap-1"
      ><input
        type="checkbox"
        class="accent-blue-500"
        bind:checked={filters.moveTutor}
      /> tutor</label
    >
    <label class="flex items-center gap-1"
      ><input
        type="checkbox"
        class="accent-blue-500"
        bind:checked={filters.moveEgg}
      /> egg</label
    >
  </div>
</fieldset>
