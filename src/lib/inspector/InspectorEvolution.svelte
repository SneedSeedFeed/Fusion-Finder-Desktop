<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import Sprite from "$lib/Sprite.svelte";
  import { formatCondition } from "$lib/inspector/format";
  import type { FusionDetail, FusionEvo } from "$lib/bindings";

  // The fusion's evolution neighbours (evolve/de-evolve one component); clicking jumps the
  // inspector to that fusion.
  let {
    detail,
    onNavigate,
  }: {
    detail: FusionDetail;
    onNavigate: (head: number, body: number) => void;
  } = $props();

  function evoSrc(e: FusionEvo): string {
    return convertFileSrc(`${e.head_dex}.${e.body_dex}.png`, "fusionsprite");
  }
</script>

{#if detail.evolves_from.length || detail.evolves_into.length}
  <section>
    <h3 class="mb-2 border-b border-gray-800 pb-1 font-semibold">Evolution</h3>
    <div class="grid gap-4 sm:grid-cols-2">
      {#each [{ title: "Evolves from", list: detail.evolves_from }, { title: "Evolves into", list: detail.evolves_into }] as col, ci (ci)}
        {#if col.list.length}
          <div>
            <div class="mb-1 text-xs text-gray-400">{col.title}</div>
            <div class="flex flex-wrap gap-2">
              {#each col.list as e, ei (ei)}
                <button
                  type="button"
                  class="flex w-24 flex-col items-center rounded-lg border border-gray-800 bg-gray-900/60 p-1 text-center hover:border-blue-500 hover:bg-gray-800"
                  onclick={() => onNavigate(e.head, e.body)}
                  title={`${e.via}${e.condition ? `: ${formatCondition(e.condition)}` : ""}`}
                >
                  <Sprite src={evoSrc(e)} size={64} alt={e.name} />
                  <span class="w-full truncate text-xs font-medium"
                    >{e.name}</span
                  >
                  <span class="text-[0.65rem] text-gray-400">
                    {e.via}{e.condition
                      ? ` · ${formatCondition(e.condition)}`
                      : ""}
                  </span>
                </button>
              {/each}
            </div>
          </div>
        {/if}
      {/each}
    </div>
  </section>
{/if}
