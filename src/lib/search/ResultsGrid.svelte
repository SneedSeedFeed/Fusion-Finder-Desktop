<script lang="ts">
  import { invoke, convertFileSrc } from "@tauri-apps/api/core";
  import { SvelteMap } from "svelte/reactivity";
  import Sprite from "$lib/Sprite.svelte";
  import { typeIcon, typeNameMap } from "$lib/typeIcon";
  import type { Bootstrap, FusionCard } from "$lib/bindings";

  // The virtualized results grid: `results` is the full (potentially 250k) list of matching fusion
  // ids, but only the rows in (and near) the viewport are rendered. Each on-screen id is hydrated
  // on demand via `fusion_cards` (name + types, decided backend-side) and cached, so we never ship
  // display data for fusions the user never scrolls to. Clicking a card opens the inspector.
  let {
    options,
    results,
    onInspect,
  }: {
    options: Bootstrap;
    results: Uint32Array;
    onInspect: (f: { head: number; body: number }) => void;
  } = $props();

  const typeNames = $derived(typeNameMap(options.types));
  function typeName(id: number): string {
    return typeNames.get(id) ?? "";
  }
  function speciesName(id: number): string {
    return options.species[id]?.name ?? `#${id}`;
  }
  // a fusion id decodes to head/body species indices (`head * species_count + body`)
  function decode(id: number): { head: number; body: number } {
    const n = options.species_count;
    return { head: Math.floor(id / n), body: id % n };
  }
  // backend serves the sprite at fusionsprite://…/{headDex}.{bodyDex}.png (dex ids, not our indices)
  function spriteUrl(head: number, body: number): string {
    const h = options.species[head]?.dex_id;
    const b = options.species[body]?.dex_id;
    return h != null && b != null
      ? convertFileSrc(`${h}.${b}.png`, "fusionsprite")
      : "";
  }

  // hydrated cards, keyed by fusion id. A fusion's name/types are fixed within a game, so this
  // persists across searches and is only cleared when the game (options) changes. `requested`
  // tracks ids already fetched or in flight so scrolling doesn't re-ask for them.
  const cards = new SvelteMap<number, FusionCard>();
  let requested = new Set<number>();

  // Cap the cache so a user who scrolls through the whole dex (250k+ fusions) doesn't grow it
  // without bound — evicted ids just re-hydrate cheaply if scrolled back to. Comfortably holds
  // many screens of history.
  const MAX_CACHED_CARDS = 5000;

  // Drop the oldest-fetched (furthest-scrolled) entries once over the cap, never anything on
  // screen. SvelteMap keeps insertion order, so iterating keys gives oldest-first. Evicted ids are
  // also dropped from `requested` so they can be fetched again later.
  function evictExcess() {
    if (cards.size <= MAX_CACHED_CARDS) return;
    const onScreen = new Set(visible);
    for (const id of [...cards.keys()]) {
      if (cards.size <= MAX_CACHED_CARDS) break;
      if (onScreen.has(id)) continue;
      cards.delete(id);
      requested.delete(id);
    }
  }

  $effect(() => {
    options; // new game -> ids mean something different, drop the cache
    cards.clear();
    requested = new Set();
  });

  const COL_W = 200; // target card width; columns auto-fit the pane width
  const ROW_H = 272; // card height (192 sprite + type row + two text lines)
  const GAP = 12;
  const OVERSCAN = 4; // extra rows above/below the viewport to keep scrolling smooth

  let scroller = $state<HTMLDivElement | null>(null);
  let scrollTop = $state(0);
  let viewportW = $state(0);
  let viewportH = $state(0);

  const cols = $derived(
    Math.max(1, Math.floor((viewportW + GAP) / (COL_W + GAP))),
  );
  const rowStride = ROW_H + GAP;
  const totalRows = $derived(Math.ceil(results.length / cols));
  const totalHeight = $derived(totalRows * rowStride);
  const firstRow = $derived(
    Math.max(0, Math.floor(scrollTop / rowStride) - OVERSCAN),
  );
  const visibleRows = $derived(Math.ceil(viewportH / rowStride) + 2 * OVERSCAN);
  const start = $derived(firstRow * cols);
  const end = $derived(
    Math.min(results.length, (firstRow + visibleRows) * cols),
  );
  const offsetY = $derived(firstRow * rowStride);
  const visible = $derived(results.slice(start, end));

  // jump back to the top whenever a new search replaces the results
  $effect(() => {
    results;
    scrollTop = 0;
    scroller?.scrollTo({ top: 0 });
  });

  // hydrate the on-screen ids we haven't asked for yet (debounced so fast scrolling doesn't spam)
  $effect(() => {
    const missing = visible.filter((id) => !requested.has(id));
    if (missing.length === 0) return;
    const handle = setTimeout(async () => {
      for (const id of missing) requested.add(id);
      try {
        const fetched = await invoke<FusionCard[]>("fusion_cards", {
          ids: missing,
        });
        for (const c of fetched) cards.set(c.id, c);
        evictExcess();
      } catch {
        for (const id of missing) requested.delete(id); // let it retry next time
      }
    }, 80);
    return () => clearTimeout(handle);
  });
</script>

<div
  bind:this={scroller}
  bind:clientWidth={viewportW}
  bind:clientHeight={viewportH}
  onscroll={(e) => (scrollTop = e.currentTarget.scrollTop)}
  class="flex-1 overflow-auto p-2"
>
  <div class="relative w-full" style="height: {totalHeight}px">
    <div
      class="absolute inset-x-0 grid"
      style="top: {offsetY}px; gap: {GAP}px; grid-template-columns: repeat({cols}, minmax(0, 1fr));"
    >
      {#each visible as id (id)}
        {@const f = decode(id)}
        {@const card = cards.get(id)}
        <button
          type="button"
          class="flex flex-col items-center overflow-hidden rounded-lg border border-gray-800 bg-gray-900/60 p-2 text-center leading-tight transition-colors hover:border-blue-500 hover:bg-gray-800"
          style="height: {ROW_H}px"
          title={`${speciesName(f.head)} / ${speciesName(f.body)}`}
          onclick={() => onInspect(f)}
        >
          <div
            class="mb-1 size-48 shrink-0 overflow-hidden rounded bg-black/30"
          >
            <Sprite src={spriteUrl(f.head, f.body)} size={192} />
          </div>
          <div class="mb-1 flex h-4 w-full items-center justify-center gap-1">
            {#each card?.types ?? [] as t (t)}
              <img
                src={typeIcon(typeName(t))}
                alt={typeName(t)}
                class="h-4 max-w-[47%] object-contain"
              />
            {/each}
          </div>
          <span class="w-full truncate text-base font-semibold text-gray-100"
            >{card?.name ?? ""}</span
          >
          <span class="w-full truncate text-xs leading-normal text-gray-400"
            >{speciesName(f.head)} / {speciesName(f.body)}</span
          >
        </button>
      {/each}
    </div>
  </div>
</div>
