<script lang="ts">
  import { typeIcon, categoryIcon, typeNameMap } from "$lib/typeIcon";
  import { MOVE_CATEGORY, MOVE_FLAGS, type NamedId } from "$lib/bindings";
  import { moveCard } from "$lib/moveCard.svelte";

  // Singleton host for the move hover-card; render once near the app root and pass the types table.
  let { types }: { types: NamedId[] } = $props();

  const typeNames = $derived(typeNameMap(types));
  function typeName(id: number): string {
    return typeNames.get(id) ?? "";
  }

  const CARD_W = 260;
  const CARD_H = 240; // estimate, only used to keep the card on-screen

  // Place the card beside the hovered row: to its left (filter sidebar + inspector both leave room),
  // flipping to the right only if there isn't space. Clamp vertically to the viewport.
  const pos = $derived.by(() => {
    const r = moveCard.rect;
    if (!r) return { left: 0, top: 0 };
    let left = r.left - CARD_W - 8;
    if (left < 8) left = Math.min(r.right + 8, window.innerWidth - CARD_W - 8);
    const top = Math.max(8, Math.min(r.top, window.innerHeight - CARD_H - 8));
    return { left, top };
  });

  function flagLabels(flags: string): string[] {
    return MOVE_FLAGS.filter(([, f]) => flags.includes(f)).map(
      ([label]) => label,
    );
  }
</script>

{#if moveCard.open && moveCard.detail}
  {@const d = moveCard.detail}
  <div
    class="pointer-events-none fixed z-60 w-65 rounded-lg border border-gray-700 bg-gray-900 p-3 text-xs text-gray-200 shadow-xl"
    style="left: {pos.left}px; top: {pos.top}px;"
    role="tooltip"
  >
    <div class="mb-1.5 flex items-center gap-1.5">
      <span class="font-semibold text-gray-100">{d.name}</span>
      <img src={typeIcon(typeName(d.ty))} alt={typeName(d.ty)} class="h-4" />
      <span class="inline-flex rounded bg-gray-700 px-1 py-0.5">
        <img
          src={categoryIcon(d.category)}
          alt={MOVE_CATEGORY[d.category]}
          class="h-3.5"
        />
      </span>
    </div>

    <div class="mb-1.5 flex gap-3 tabular-nums text-gray-400">
      <span>Pow <span class="text-gray-200">{d.power ?? "—"}</span></span>
      <span>Acc <span class="text-gray-200">{d.accuracy ?? "—"}</span></span>
      <span>PP <span class="text-gray-200">{d.pp}</span></span>
      {#if d.effect_chance !== null}
        <span>Eff <span class="text-gray-200">{d.effect_chance}%</span></span>
      {/if}
      {#if d.priority !== 0}
        <span
          >Prio <span class="text-gray-200"
            >{d.priority > 0 ? `+${d.priority}` : d.priority}</span
          ></span
        >
      {/if}
    </div>

    {#if flagLabels(d.flags).length}
      <div class="mb-1.5 flex flex-wrap gap-1">
        {#each flagLabels(d.flags) as label (label)}
          <span class="rounded-full bg-gray-800 px-1.5 py-0.5 text-gray-400"
            >{label}</span
          >
        {/each}
      </div>
    {/if}

    <p class="text-gray-400">{d.description}</p>

    {#if d.machine}
      <div class="mt-2 border-t border-gray-800 pt-2">
        <span class="font-medium text-gray-300"
          >{d.machine.name}{d.machine.is_hm ? " (HM)" : ""} 💿
        </span>
        {#if d.machine.locations.length}
          <ul class="mt-0.5 text-gray-400">
            {#each d.machine.locations as loc, li (li)}
              <li>{loc}</li>
            {/each}
          </ul>
        {:else}
          <span class="text-gray-500">
            — buy at a Mart or earn from an event</span
          >
        {/if}
      </div>
    {/if}

    {#if d.tutor_locations.length}
      <div class="mt-2 border-t border-gray-800 pt-2">
        <span class="font-medium text-gray-300">Move tutor 👤</span>
        <ul class="mt-0.5 text-gray-400">
          {#each d.tutor_locations as loc, li (li)}
            <li>{loc}</li>
          {/each}
        </ul>
      </div>
    {/if}

    {#if d.expert}
      <div class="mt-2 border-t border-gray-800 pt-2">
        <span class="font-medium text-gray-300"
          >{d.expert.legendary ? "Legendary Move Expert" : "Move Expert"} ✨</span
        >
        <p class="mt-0.5 text-gray-400">
          Taught to {d.expert.condition}.
        </p>
        {#if d.expert.locations.length}
          <ul class="mt-0.5 text-gray-400">
            {#each d.expert.locations as loc, li (li)}
              <li>{loc}</li>
            {/each}
          </ul>
        {/if}
      </div>
    {/if}
  </div>
{/if}
