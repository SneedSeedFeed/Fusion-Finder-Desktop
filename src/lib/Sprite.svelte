<script lang="ts">
  import RingLoader from "$lib/RingLoader.svelte";

  let {
    src,
    size = 96,
    alt = "",
    // nearest-neighbour is crisp at 1:1 / integer scales; smooth avoids the uneven aliasing you
    // get upscaling 96px art by a fractional factor (e.g. the 120px grid cells)
    pixelated = true,
  }: {
    src: string;
    size?: number;
    alt?: string;
    pixelated?: boolean;
  } = $props();

  let status = $state<"loading" | "loaded" | "error">("loading");
  // reset to loading whenever the source changes (e.g. cycling sprite variants)
  $effect(() => {
    src;
    status = "loading";
  });
</script>

<div
  class="relative grid place-items-center overflow-hidden"
  style="width: {size}px; height: {size}px"
>
  {#if status === "loading"}
    <div class="absolute">
      <RingLoader
        size={Math.round(size * 0.5).toString()}
        color="#9ca3af"
        duration="1.3s"
      />
    </div>
  {/if}
  {#if status !== "error"}
    <img
      {src}
      {alt}
      loading="lazy"
      decoding="async"
      class="size-full object-contain {pixelated
        ? '[image-rendering:pixelated]'
        : ''} {status === 'loaded' ? '' : 'invisible'}"
      onload={() => (status = "loaded")}
      onerror={() => (status = "error")}
    />
  {/if}
</div>
