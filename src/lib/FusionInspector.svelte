<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { typeNameMap } from "$lib/typeIcon";
  import type { FusionDetail, NamedId } from "$lib/bindings";
  import type { Favourites } from "$lib/favourites.svelte";
  import InspectorSprite from "$lib/inspector/InspectorSprite.svelte";
  import InspectorEvolution from "$lib/inspector/InspectorEvolution.svelte";
  import InspectorStats from "$lib/inspector/InspectorStats.svelte";
  import InspectorAbilities from "$lib/inspector/InspectorAbilities.svelte";
  import InspectorMatchups from "$lib/inspector/InspectorMatchups.svelte";
  import InspectorEvoLines from "$lib/inspector/InspectorEvoLines.svelte";
  import InspectorMoves from "$lib/inspector/InspectorMoves.svelte";

  let {
    head,
    body,
    types,
    favourites,
    onClose,
  }: {
    head: number;
    body: number;
    // the bootstrap types table, for resolving the type ids the backend sends
    types: NamedId[];
    favourites: Favourites;
    onClose: () => void;
  } = $props();

  const typeNames = $derived(typeNameMap(types));
  function typeName(id: number): string {
    return typeNames.get(id) ?? "";
  }

  // seeded from the props once; flip/navigate then drive them locally
  // svelte-ignore state_referenced_locally
  let h = $state(head);
  // svelte-ignore state_referenced_locally
  let b = $state(body);
  let detail = $state<FusionDetail | null>(null);
  let error = $state<string | null>(null);

  // live favourite state for the currently-shown fusion (reactive on both detail + the set)
  const isFavourite = $derived(
    detail ? favourites.has(detail.head.dex_id, detail.body.dex_id) : false,
  );
  function toggleFavourite() {
    if (detail) favourites.toggle(detail.head.dex_id, detail.body.dex_id);
  }

  $effect(() => {
    const [hh, bb] = [h, b];
    detail = null;
    invoke<FusionDetail>("fusion_detail", { head: hh, body: bb })
      .then((d) => (detail = d))
      .catch((e) => (error = String(e)));
  });

  function flip() {
    [h, b] = [b, h];
  }
  // jump the inspector to another fusion (its evolution neighbours)
  function navigate(head: number, body: number) {
    [h, b] = [head, body];
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onClose()} />

<!-- backdrop: clicking outside the panel closes (Escape is handled on window) -->
<div
  class="fixed inset-0 z-50 flex items-start justify-center overflow-auto bg-black/50 p-4"
  role="presentation"
  onclick={(e) => e.target === e.currentTarget && onClose()}
>
  <!-- panel -->
  <div
    class="my-4 w-full max-w-3xl rounded-lg border border-gray-800 bg-gray-900 text-gray-200 shadow-xl"
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    {#if error}
      <p class="p-6 text-red-400">{error}</p>
    {:else if !detail}
      <p class="p-6 text-gray-400">Loading…</p>
    {:else}
      <header
        class="flex items-center gap-3 border-b border-gray-800 px-4 py-3"
      >
        <div>
          <div class="text-lg font-semibold">{detail.fusion_name}</div>
          <div class="text-xs text-gray-500">
            #{detail.head.dex_id}.{detail.body.dex_id} · {detail.head.name} / {detail
              .body.name}
          </div>
        </div>
        <div class="ml-auto flex items-center gap-2">
          <button
            class="rounded border border-gray-700 px-3 py-1 text-sm hover:bg-gray-700 {isFavourite
              ? 'border-yellow-500/60 bg-yellow-500/10 text-yellow-400'
              : 'bg-gray-800 text-gray-300'}"
            aria-pressed={isFavourite}
            title={isFavourite ? "Remove from favourites" : "Add to favourites"}
            onclick={toggleFavourite}
          >
            {isFavourite ? "★ Favourited" : "☆ Favourite"}
          </button>
          <button
            class="rounded border border-gray-700 bg-gray-800 px-3 py-1 text-sm hover:bg-gray-700"
            onclick={flip}
          >
            ⇄ Flip
          </button>
          <button
            class="rounded border border-gray-700 bg-gray-800 px-3 py-1 text-sm hover:bg-gray-700"
            onclick={onClose}
          >
            ✕
          </button>
        </div>
      </header>

      <div class="space-y-5 p-4">
        <InspectorSprite {detail} {typeName} />
        <InspectorEvolution {detail} onNavigate={navigate} />
        <InspectorStats {detail} />
        <InspectorAbilities {detail} />
        <InspectorMatchups {detail} {typeName} />
        <InspectorEvoLines {detail} {typeName} />
        <InspectorMoves {detail} {typeName} />
      </div>
    {/if}
  </div>
</div>
