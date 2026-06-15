<script lang="ts">
  import RangeSlider from "$lib/search/RangeSlider.svelte";
  import { STATS, type StatBounds, type StatKey } from "$lib/bindings";

  // One range slider per base stat. `value` is the bound per-stat [min, max] position; a stat is
  // highlighted as active when its slider has moved off the full bounds.
  let {
    bounds,
    value = $bindable(),
  }: {
    bounds: StatBounds;
    value: Record<StatKey, [number, number]>;
  } = $props();
</script>

{#each STATS as s (s)}
  {@const b = bounds[s]}
  {@const active = value[s][0] > b.min || value[s][1] < b.max}
  <div class="mb-3">
    <div class="mb-1 flex items-center justify-between text-xs">
      <span class="font-medium {active ? 'text-blue-400' : 'text-gray-400'}"
        >{s.toUpperCase()}</span
      >
      <span class="tabular-nums text-gray-400"
        >{value[s][0]} – {value[s][1]}</span
      >
    </div>
    <div class="px-2">
      <RangeSlider min={b.min} max={b.max} bind:value={value[s]} />
    </div>
  </div>
{/each}
