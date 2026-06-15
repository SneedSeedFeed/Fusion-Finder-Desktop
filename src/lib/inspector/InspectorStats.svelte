<script lang="ts">
  import type { FusionDetail } from "$lib/bindings";
  import { STAT_LABELS } from "$lib/inspector/format";

  // The fused stat line, each stat compared (delta) against both parents.
  let { detail }: { detail: FusionDetail } = $props();

  function fmtDelta(n: number): string {
    return n > 0 ? `+${n}` : `${n}`;
  }
</script>

<section>
  <h3 class="mb-1 border-b border-gray-800 pb-1 font-semibold">Stats</h3>
  <div class="space-y-0.5">
    {#each detail.stats as s, i (i)}
      <div
        class="grid grid-cols-[5rem_3rem_1fr_1fr] items-center gap-2 text-sm"
      >
        <span class="text-gray-500">{STAT_LABELS[i]}</span>
        <span class="text-right font-semibold tabular-nums">{s.value}</span>
        {#each [{ d: s.value - s.head, who: detail.head.name }, { d: s.value - s.body, who: detail.body.name }] as cmp, i (i)}
          <span
            class="tabular-nums {cmp.d > 0
              ? 'text-green-400'
              : cmp.d < 0
                ? 'text-red-400'
                : 'text-gray-400'}"
          >
            {fmtDelta(cmp.d)}
            <span class="text-[0.65rem] text-gray-400">vs {cmp.who}</span>
          </span>
        {/each}
      </div>
    {/each}
  </div>
</section>
