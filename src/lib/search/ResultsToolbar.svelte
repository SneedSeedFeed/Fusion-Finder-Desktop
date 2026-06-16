<script lang="ts">
  import {
    SYNERGY_METRICS,
    SYNERGY_STATS,
    type Metric,
    type SynergyStat,
  } from "$lib/bindings";
  import MetricPicker from "$lib/search/MetricPicker.svelte";

  // Result count + sort controls + the change-game button. Sort state is bound back to the page.
  // `metric === null` means dex order; when a metric is chosen a second (denominator) metric can be
  // picked to sort by the ratio between the two.
  let {
    count,
    searching,
    metric = $bindable(),
    metric2 = $bindable(),
    sortDesc = $bindable(),
    synergyStats = $bindable(),
    version,
    onChangeGame,
    onOpenAreas,
  }: {
    count: number;
    searching: boolean;
    metric: Metric | null;
    metric2: Metric | null;
    sortDesc: boolean;
    synergyStats: SynergyStat[];
    version: string;
    onChangeGame: () => void;
    onOpenAreas: () => void;
  } = $props();

  // dropping back to dex order, or picking the same metric on both sides, leaves no meaningful
  // ratio, so clear the denominator.
  $effect(() => {
    if (metric === null || metric === metric2) metric2 = null;
  });

  // the per-stat chips only matter for the synergy metrics
  const showSynergyStats = $derived(
    metric !== null && SYNERGY_METRICS.has(metric),
  );

  function toggleStat(s: SynergyStat) {
    synergyStats = synergyStats.includes(s)
      ? synergyStats.filter((x) => x !== s)
      : [...synergyStats, s];
  }
</script>

<header class="flex items-center gap-2 border-b border-gray-800 px-3 py-2">
  <span
    ><strong class="text-gray-100">{count.toLocaleString()}</strong> fusions</span
  >
  {#if searching}<span class="text-gray-500">· searching…</span>{/if}
  <div class="ml-auto flex items-center gap-1 text-xs text-gray-400">
    Sort
    <MetricPicker bind:value={metric} nullLabel="Dex order" />
  </div>
  {#if metric !== null}
    <div class="flex items-center gap-1 text-xs text-gray-400">
      ÷
      <MetricPicker
        bind:value={metric2}
        nullLabel="—"
        exclude={metric}
        title="Sort by the ratio of the two metrics"
      />
    </div>
  {/if}
  {#if showSynergyStats}
    <div
      class="flex items-center gap-0.5"
      title="Stats counted toward synergy (click to include/exclude)"
    >
      {#each SYNERGY_STATS as s (s.value)}
        <button
          type="button"
          aria-pressed={synergyStats.includes(s.value)}
          onclick={() => toggleStat(s.value)}
          class="rounded border px-1.5 py-1 text-xs {synergyStats.includes(
            s.value,
          )
            ? 'border-blue-500 bg-blue-600 text-white'
            : 'border-gray-700 bg-gray-800 text-gray-500 hover:text-gray-300'}"
        >
          {s.label}
        </button>
      {/each}
    </div>
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
