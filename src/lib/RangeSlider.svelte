<script lang="ts">
  import { createSlider, melt } from "@melt-ui/svelte";

  let {
    min,
    max,
    step = 1,
    value = $bindable(),
  }: { min: number; max: number; step?: number; value: number[] } = $props();

  // Uncontrolled: melt owns the value, seeded from the initial `value`, and pushes changes
  // back up through `onValueChange`. Bounds are fixed for this slider's lifetime.
  // svelte-ignore state_referenced_locally
  const {
    elements: { root, range, thumbs },
  } = createSlider({
    defaultValue: value,
    min,
    max,
    step,
    onValueChange: ({ next }) => {
      if (next.length === 2 && (next[0] !== value[0] || next[1] !== value[1])) {
        value = [...next];
      }
      return next;
    },
  });
</script>

<span use:melt={$root} class="relative flex h-4 w-full items-center">
  <span class="block h-1 w-full rounded-full bg-gray-200">
    <span use:melt={$range} class="h-1 rounded-full bg-blue-500"></span>
  </span>
  {#each $thumbs as thumb}
    <span
      use:melt={thumb}
      class="block size-3 cursor-grab rounded-full bg-blue-600 focus:ring-2 focus:ring-blue-300 focus:outline-none active:cursor-grabbing"
    ></span>
  {/each}
</span>
