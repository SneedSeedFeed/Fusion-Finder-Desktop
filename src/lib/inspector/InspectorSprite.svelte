<script lang="ts">
  import { convertFileSrc } from "@tauri-apps/api/core";
  import Sprite from "$lib/Sprite.svelte";
  import { typeIcon, typeIds } from "$lib/typeIcon";
  import type { FusionDetail, SpriteVariant } from "$lib/bindings";

  // The sprite (click to cycle hand-drawn variants), its artist credit, and the fusion's types.
  let {
    detail,
    typeName,
  }: { detail: FusionDetail; typeName: (id: number) => string } = $props();

  let spriteIdx = $state(0);
  // always show at least the canonical sprite, even when there are no hand-drawn variants
  const spriteList = $derived<SpriteVariant[]>(
    detail.sprites.length ? detail.sprites : [{ variant: "", artist: null }],
  );
  const current = $derived(spriteList[spriteIdx % spriteList.length]);

  function spriteSrc(variant: string): string {
    return convertFileSrc(
      `${detail.head.dex_id}.${detail.body.dex_id}${variant}.png`,
      "fusionsprite",
    );
  }
  function cycleSprite() {
    if (spriteList.length > 1) spriteIdx = (spriteIdx + 1) % spriteList.length;
  }
</script>

<section class="flex flex-col items-center gap-2">
  <button
    class="rounded bg-black/30 p-2 {spriteList.length > 1
      ? 'cursor-pointer ring-1 ring-gray-700 hover:ring-blue-400'
      : 'cursor-default'}"
    onclick={cycleSprite}
    title={spriteList.length > 1 ? "Click to cycle sprites" : ""}
  >
    <Sprite
      src={spriteSrc(current.variant)}
      size={192}
      alt={detail.fusion_name}
    />
  </button>
  <div class="text-center text-xs text-gray-500">
    {#if current.artist}art by <span class="font-medium text-gray-200"
        >{current.artist}</span
      >{:else}auto-generated{/if}
    {#if spriteList.length > 1}<span class="ml-1 text-gray-400"
        >({spriteIdx + 1}/{spriteList.length})</span
      >{/if}
  </div>
  <div class="flex gap-1">
    {#each typeIds(detail.types) as t (t)}
      <img src={typeIcon(typeName(t))} alt={typeName(t)} class="h-5" />
    {/each}
  </div>
</section>
