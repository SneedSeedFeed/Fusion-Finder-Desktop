<script lang="ts">
  import { typeIcon, categoryIcon } from "$lib/typeIcon";
  import { learnLabels } from "$lib/inspector/format";
  import { showMoveCard, hideMoveCard } from "$lib/moveCard.svelte";
  import { MOVE_CATEGORY, type FusionDetail } from "$lib/bindings";

  // The fusion's combined move pool, with how each move is learned.
  let {
    detail,
    typeName,
  }: { detail: FusionDetail; typeName: (id: number) => string } = $props();
</script>

<section>
  <h3 class="mb-1 border-b border-gray-800 pb-1 font-semibold">
    Moves <span class="font-normal text-gray-400">({detail.moves.length})</span>
  </h3>
  <table class="w-full text-left text-xs">
    <thead class="text-gray-400">
      <tr>
        <th class="py-1 font-medium">Learned</th>
        <th class="font-medium">Move</th>
        <th class="font-medium">Type</th>
        <th class="font-medium">Cat.</th>
        <th class="text-right font-medium">Pow</th>
        <th class="text-right font-medium">Acc</th>
        <th class="text-right font-medium">PP</th>
      </tr>
    </thead>
    <tbody>
      {#each detail.moves as m (m.name)}
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
      {/each}
    </tbody>
  </table>
</section>
