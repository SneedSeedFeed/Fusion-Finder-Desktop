<script lang="ts">
  import { typeIcon } from "$lib/typeIcon";
  import { multiplierGlyph } from "$lib/inspector/format";
  import type { FusionDetail } from "$lib/bindings";

  // Defensive matchups grouped by multiplier (weaknesses & resistances), strongest first.
  let {
    detail,
    typeName,
  }: { detail: FusionDetail; typeName: (id: number) => string } = $props();
</script>

{#if detail.matchups.length}
  <section>
    <h3 class="mb-1 border-b border-gray-800 pb-1 font-semibold">
      Weaknesses & Resistances
    </h3>
    <div class="space-y-1">
      {#each detail.matchups as m (m.multiplier)}
        <div class="flex items-center gap-2">
          <span
            class="w-8 shrink-0 text-right text-sm font-semibold text-gray-300"
            >{multiplierGlyph(m.multiplier)}</span
          >
          <div class="flex flex-wrap gap-1">
            {#each m.types as t (t)}<img
                src={typeIcon(typeName(t))}
                alt={typeName(t)}
                class="h-5"
              />{/each}
          </div>
        </div>
      {/each}
    </div>
  </section>
{/if}
