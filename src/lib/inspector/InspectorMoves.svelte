<script lang="ts">
  import { typeIcon, categoryIcon } from "$lib/typeIcon";
  import { learnLabels } from "$lib/inspector/format";
  import { showMoveCard, hideMoveCard } from "$lib/moveCard.svelte";
  import { MOVE_CATEGORY, type FusionDetail } from "$lib/bindings";
  import RangeSlider from "$lib/search/RangeSlider.svelte";

  // The fusion's combined move pool, with how each move is learned.
  let {
    detail,
    typeName,
  }: { detail: FusionDetail; typeName: (id: number) => string } = $props();

  function titleCase(s: string): string {
    return s.charAt(0) + s.slice(1).toLowerCase();
  }

  const fusionTypes = $derived(
    detail.types.filter((t): t is number => t !== null),
  );

  // slider bounds for the numeric filters (0 up to the strongest in the pool)
  const powerMax = $derived(
    Math.max(0, ...detail.moves.map((m) => m.power ?? 0)),
  );
  const accuracyMax = $derived(
    Math.max(0, ...detail.moves.map((m) => m.accuracy ?? 0)),
  );
  const ppMax = $derived(Math.max(0, ...detail.moves.map((m) => m.pp)));

  // distinct move types as {id, name}, ordered by name, for the type dropdown
  const moveTypes = $derived(
    [...new Set(detail.moves.map((m) => m.ty))]
      .map((id) => ({ id, name: typeName(id) }))
      .sort((a, b) => a.name.localeCompare(b.name)),
  );

  let search = $state("");
  let typeFilter = $state<number | "STAB" | null>(null);
  let categoryFilter = $state<0 | 1 | 2 | null>(null);
  let power = $state<[number, number]>([0, 0]);
  let accuracy = $state<[number, number]>([0, 0]);
  let pp = $state<[number, number]>([0, 0]);

  // Reset every filter whenever the move pool changes
  $effect(() => {
    detail.moves;
    search = "";
    typeFilter = null;
    categoryFilter = null;
    power = [0, powerMax];
    accuracy = [0, accuracyMax];
    pp = [0, ppMax];
    sortCol = null;
    sortDir = 1;
  });

  // column sorting
  type SortCol =
    | "learned"
    | "name"
    | "type"
    | "category"
    | "power"
    | "accuracy"
    | "pp";
  let sortCol = $state<SortCol | null>(null);
  let sortDir = $state<1 | -1>(1);

  function toggleSort(col: SortCol) {
    if (sortCol === col) sortDir = sortDir === 1 ? -1 : 1;
    else {
      sortCol = col;
      sortDir = 1;
    }
  }

  function sortValue(m: FusionDetail["moves"][number], col: SortCol) {
    switch (col) {
      case "learned":
        return learnLabels(m.sources).join(", ");
      case "name":
        return m.name;
      case "type":
        return typeName(m.ty);
      case "category":
        return m.category;
      case "power":
        return m.power;
      case "accuracy":
        return m.accuracy;
      case "pp":
        return m.pp;
    }
  }

  const filtered = $derived.by(() => {
    const q = search.trim().toLowerCase();
    return detail.moves.filter(
      (m) =>
        (typeFilter === null ||
          (typeFilter === "STAB"
            ? fusionTypes.includes(m.ty)
            : m.ty === typeFilter)) &&
        (categoryFilter === null || m.category === categoryFilter) &&
        (m.power ?? 0) >= power[0] &&
        (m.power ?? 0) <= power[1] &&
        // a null accuracy never misses — treat it as the top of the range
        (m.accuracy ?? accuracyMax) >= accuracy[0] &&
        (m.accuracy ?? accuracyMax) <= accuracy[1] &&
        m.pp >= pp[0] &&
        m.pp <= pp[1] &&
        (!q ||
          m.name.toLowerCase().includes(q) ||
          m.description.toLowerCase().includes(q)),
    );
  });

  const sorted = $derived.by(() => {
    if (sortCol === null) return filtered;
    const col = sortCol;
    return [...filtered].sort((a, b) => {
      const va = sortValue(a, col);
      const vb = sortValue(b, col);
      // nulls always sort to the end, regardless of direction
      if (va == null && vb == null) return 0;
      if (va == null) return 1;
      if (vb == null) return -1;
      const c =
        typeof va === "string"
          ? va.localeCompare(vb as string)
          : va - (vb as number);
      return c * sortDir;
    });
  });

  const active = $derived(
    search.trim() !== "" ||
      typeFilter !== null ||
      categoryFilter !== null ||
      power[0] > 0 ||
      power[1] < powerMax ||
      accuracy[0] > 0 ||
      accuracy[1] < accuracyMax ||
      pp[0] > 0 ||
      pp[1] < ppMax,
  );

  function clear() {
    search = "";
    typeFilter = null;
    categoryFilter = null;
    power = [0, powerMax];
    accuracy = [0, accuracyMax];
    pp = [0, ppMax];
  }
</script>

<section>
  <h3
    class="mb-1 flex items-center gap-2 border-b border-gray-800 pb-1 font-semibold"
  >
    Moves
    <span class="font-normal text-gray-400"
      >({filtered.length}{filtered.length !== detail.moves.length
        ? ` of ${detail.moves.length}`
        : ""})</span
    >
    {#if active}
      <button
        type="button"
        class="ml-auto rounded border border-gray-700 bg-gray-800 px-2 py-0.5 text-xs font-normal text-gray-300 hover:bg-gray-700"
        onclick={clear}>Clear filters</button
      >
    {/if}
  </h3>

  <!-- filter bar over the move pool (client-side; the pool is small) -->
  <div class="mb-2 space-y-1">
    <div class="flex gap-1">
      <input
        type="search"
        placeholder="search"
        bind:value={search}
        class="min-w-0 flex-1 rounded border border-gray-700 bg-gray-800 p-1 text-xs text-gray-200 placeholder:text-gray-500"
      />
      <select
        class="min-w-0 flex-1 rounded border border-gray-700 bg-gray-800 p-1 text-xs text-gray-200"
        bind:value={typeFilter}
      >
        <option value={null}>any type</option>
        {#if fusionTypes.length}
          <option value="STAB"
            >STAB ({fusionTypes
              .map((t) => titleCase(typeName(t)))
              .join("/")})</option
          >
        {/if}
        {#each moveTypes as t (t.id)}
          <option value={t.id}>{titleCase(t.name)}</option>
        {/each}
      </select>
      <select
        class="min-w-0 flex-1 rounded border border-gray-700 bg-gray-800 p-1 text-xs text-gray-200"
        bind:value={categoryFilter}
      >
        <option value={null}>any category</option>
        <option value={0}>Physical</option>
        <option value={1}>Special</option>
        <option value={2}>Status</option>
      </select>
    </div>

    <!-- power / accuracy / pp range sliders (same vein as the search-side filters) -->
    <div class="grid grid-cols-1 gap-x-3 gap-y-1 sm:grid-cols-3">
      <div>
        <div class="mb-0.5 flex items-center justify-between text-xs">
          <span
            class="font-medium {power[0] > 0 || power[1] < powerMax
              ? 'text-blue-400'
              : 'text-gray-400'}">Power</span
          >
          <span class="tabular-nums text-gray-400">{power[0]} – {power[1]}</span
          >
        </div>
        <div class="px-2">
          <RangeSlider min={0} max={powerMax} bind:value={power} />
        </div>
      </div>
      <div>
        <div class="mb-0.5 flex items-center justify-between text-xs">
          <span
            class="font-medium {accuracy[0] > 0 || accuracy[1] < accuracyMax
              ? 'text-blue-400'
              : 'text-gray-400'}">Accuracy</span
          >
          <span class="tabular-nums text-gray-400"
            >{accuracy[0]} – {accuracy[1]}</span
          >
        </div>
        <div class="px-2">
          <RangeSlider min={0} max={accuracyMax} bind:value={accuracy} />
        </div>
      </div>
      <div>
        <div class="mb-0.5 flex items-center justify-between text-xs">
          <span
            class="font-medium {pp[0] > 0 || pp[1] < ppMax
              ? 'text-blue-400'
              : 'text-gray-400'}">PP</span
          >
          <span class="tabular-nums text-gray-400">{pp[0]} – {pp[1]}</span>
        </div>
        <div class="px-2">
          <RangeSlider min={0} max={ppMax} bind:value={pp} />
        </div>
      </div>
    </div>
  </div>

  {#snippet headCell(col: SortCol, label: string, alignRight: boolean)}
    <th
      class="py-1 font-medium {alignRight ? 'text-right' : ''}"
      aria-sort={sortCol === col
        ? sortDir === 1
          ? "ascending"
          : "descending"
        : "none"}
    >
      <button
        type="button"
        class="inline-flex items-center gap-0.5 hover:text-gray-200 {alignRight
          ? 'flex-row-reverse'
          : ''} {sortCol === col ? 'text-blue-400' : ''}"
        onclick={() => toggleSort(col)}
      >
        {label}<span class="w-2 text-blue-400"
          >{sortCol === col ? (sortDir === 1 ? "▲" : "▼") : ""}</span
        >
      </button>
    </th>
  {/snippet}

  <table class="w-full text-left text-xs">
    <thead class="text-gray-400">
      <tr>
        {@render headCell("learned", "Learned", false)}
        {@render headCell("name", "Move", false)}
        {@render headCell("type", "Type", false)}
        {@render headCell("category", "Cat.", false)}
        {@render headCell("power", "Pow", true)}
        {@render headCell("accuracy", "Acc", true)}
        {@render headCell("pp", "PP", true)}
      </tr>
    </thead>
    <tbody>
      {#each sorted as m (m.name)}
        <tr
          class="border-t border-gray-800 hover:bg-gray-800/40"
          onmouseenter={(e) => showMoveCard(m.id, e.currentTarget)}
          onmouseleave={hideMoveCard}
        >
          <td class="py-1 pr-2 whitespace-nowrap text-gray-500"
            >{learnLabels(m.sources).join(", ")}</td
          >
          <td class="pr-2 font-medium">{m.name}</td>
          <td class="pr-2"
            ><img
              src={typeIcon(typeName(m.ty))}
              alt={typeName(m.ty)}
              class="h-4"
            /></td
          >
          <td class="pr-2">
            <span
              class="inline-flex rounded bg-gray-700 px-1 py-0.5"
              title={MOVE_CATEGORY[m.category]}
            >
              <img
                src={categoryIcon(m.category)}
                alt={MOVE_CATEGORY[m.category]}
                class="h-3.5"
              />
            </span>
          </td>
          <td class="text-right tabular-nums">{m.power ?? "-"}</td>
          <td class="text-right tabular-nums">{m.accuracy ?? "-"}</td>
          <td class="text-right tabular-nums">{m.pp}</td>
        </tr>
      {:else}
        <tr>
          <td colspan="7" class="py-3 text-center text-gray-500"
            >No moves match these filters.</td
          >
        </tr>
      {/each}
    </tbody>
  </table>
</section>
