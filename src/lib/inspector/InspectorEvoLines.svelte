<script lang="ts">
  import { typeIcon, typeIds } from "$lib/typeIcon";
  import { formatCondition, methodLabel } from "$lib/inspector/format";
  import type { FusionDetail } from "$lib/bindings";

  // Both parents' full evolution families, base-first, with where each member is found in the wild.
  let {
    detail,
    typeName,
  }: { detail: FusionDetail; typeName: (id: number) => string } = $props();
</script>

<section class="grid gap-4 sm:grid-cols-2">
  {#each [{ title: detail.head.name, line: detail.head_line }, { title: detail.body.name, line: detail.body_line }] as fam, fi (fi)}
    <div>
      <h3 class="mb-1 border-b border-gray-800 pb-1 font-semibold">
        {fam.title} line
      </h3>
      <div class="space-y-3 text-sm">
        {#each fam.line as node, ni (ni)}
          <div>
            <div class="flex items-center gap-2">
              {#if node.from_condition}<span class="text-xs text-gray-400"
                  >→ {formatCondition(node.from_condition)}</span
                >{/if}
              <span class="font-medium">{node.name}</span>
              {#each typeIds(node.types) as t, ti (ti)}<img
                  src={typeIcon(typeName(t))}
                  alt={typeName(t)}
                  class="h-4"
                />{/each}
            </div>
            {#if node.encounters.length}
              <table class="mt-1 w-full text-left text-xs">
                <thead class="text-gray-400">
                  <tr>
                    <th class="font-medium">Location</th>
                    <th class="font-medium">How</th>
                    <th class="font-medium whitespace-nowrap">Lv.</th>
                    <th class="text-right font-medium">%</th>
                  </tr>
                </thead>
                <tbody>
                  {#each node.encounters as e, ei (ei)}
                    <tr class="border-t border-gray-800">
                      <td class="pr-2">
                        {e.location}
                        {#if e.mode !== "Both"}
                          <span
                            class="ml-1 rounded px-1 text-[10px] {e.mode ===
                            'Classic'
                              ? 'bg-amber-900/60 text-amber-200'
                              : 'bg-violet-900/60 text-violet-200'}"
                            title="Only in {e.mode} mode">{e.mode}</span
                          >
                        {/if}
                      </td>
                      <td class="pr-2 text-gray-500">{methodLabel(e.method)}</td
                      >
                      <td class="pr-2 tabular-nums whitespace-nowrap"
                        >{e.min_level === e.max_level
                          ? e.min_level
                          : `${e.min_level}–${e.max_level}`}</td
                      >
                      <td class="text-right tabular-nums"
                        >{e.method === "Static" ||
                        e.method === "Gift" ||
                        e.method === "Roaming"
                          ? "—"
                          : `${e.chance}%`}</td
                      >
                    </tr>
                  {/each}
                </tbody>
              </table>
            {:else}
              <div class="mt-0.5 text-xs text-gray-400 italic">
                not found in the wild
              </div>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  {/each}
</section>
