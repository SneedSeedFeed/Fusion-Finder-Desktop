<script lang="ts">
  import { METRICS, type Metric } from "$lib/bindings";

  // Result count + sort controls + the change-game button. Sort state is bound back to the page.
  // `metric === null` means dex order; when a metric is chosen a second (denominator) metric can be
  // picked to sort by the ratio between the two.
  let {
    count,
    searching,
    metric = $bindable(),
    metric2 = $bindable(),
    sortDesc = $bindable(),
    version,
    onChangeGame,
    onOpenAreas,
  }: {
    count: number;
    searching: boolean;
    metric: Metric | null;
    metric2: Metric | null;
    sortDesc: boolean;
    version: string;
    onChangeGame: () => void;
    onOpenAreas: () => void;
  } = $props();

  // dropping back to dex order or picking the same metric on both sides leaves no meaningful ratio, so clear the denominator.
  function onPrimaryChange() {
    if (metric === null || metric === metric2) metric2 = null;
  }
</script>

<header class="flex items-center gap-2 border-b border-gray-800 px-3 py-2">
  <span
    ><strong class="text-gray-100">{count.toLocaleString()}</strong> fusions</span
  >
  {#if searching}<span class="text-gray-500">· searching…</span>{/if}
  <label class="ml-auto flex items-center gap-1 text-xs text-gray-400">
    Sort
    <select
      class="rounded border border-gray-700 bg-gray-800 p-1 text-gray-200"
      bind:value={metric}
      onchange={onPrimaryChange}
    >
      <option value={null}>Dex order</option>
      {#each METRICS as m (m.value)}<option value={m.value}>{m.label}</option
        >{/each}
    </select>
  </label>
  {#if metric !== null}
    <label class="flex items-center gap-1 text-xs text-gray-400">
      ÷
      <select
        class="rounded border border-gray-700 bg-gray-800 p-1 text-gray-200"
        bind:value={metric2}
        title="Sort by the ratio of the two metrics"
      >
        <option value={null}>—</option>
        {#each METRICS as m (m.value)}
          {#if m.value !== metric}<option value={m.value}>{m.label}</option
            >{/if}
        {/each}
      </select>
    </label>
  {/if}
  <button
    type="button"
    class="rounded border border-gray-700 bg-gray-800 px-2 py-1 text-gray-200 hover:bg-gray-700"
    title={sortDesc ? "Descending" : "Ascending"}
    aria-label={sortDesc ? "Descending" : "Ascending"}
    onclick={() => (sortDesc = !sortDesc)}>{sortDesc ? "↓" : "↑"}</button
  >
  <button
    type="button"
    class="rounded border border-gray-700 bg-gray-800 px-2 py-1 text-xs text-gray-300 hover:bg-gray-700"
    title="What's on a route?"
    onclick={onOpenAreas}>Areas 🗺️</button
  >
  <button
    type="button"
    class="rounded border border-gray-700 bg-gray-800 px-2 py-1 text-xs text-gray-300 hover:bg-gray-700"
    title="Change game folder or version"
    onclick={onChangeGame}>{version} ⚙</button
  >
</header>
