<script lang="ts">
  import { computePosition, flip, shift, autoUpdate } from "@floating-ui/dom";
  import type { Metric, MetricGroup } from "$lib/bindings";

  // One category's submenu, positioned to the right of its row with Floating UI (flips/shifts near
  // the screen edge). Open-state is owned by the parent `MetricPicker`, so this is purely "render +
  // position + emit a pick". Tagged `data-submenu={index}` for the parent's safe-polygon geometry.
  let {
    reference,
    group,
    index,
    exclude = null,
    onPick,
  }: {
    reference: HTMLElement | undefined;
    group: MetricGroup;
    index: number;
    exclude?: Metric | null;
    onPick: (m: Metric) => void;
  } = $props();

  let el = $state<HTMLElement>();
  let ready = $state(false);

  $effect(() => {
    const ref = reference;
    const node = el;
    if (!ref || !node) return;
    return autoUpdate(ref, node, () => {
      computePosition(ref, node, {
        strategy: "fixed",
        placement: "right-start",
        middleware: [flip({ padding: 4 }), shift({ padding: 4 })],
      }).then(({ x, y }) => {
        node.style.left = `${x}px`;
        node.style.top = `${y}px`;
        ready = true;
      });
    });
  });

  const pickKey = (e: KeyboardEvent, m: Metric) => {
    if (e.key === "Enter" || e.key === " ") onPick(m);
  };
</script>

<div
  bind:this={el}
  data-submenu={index}
  style="position: fixed; left: 0; top: 0;"
  class="z-40 w-36 rounded border border-gray-700 bg-gray-800 p-1 text-gray-200 shadow-lg transition-opacity {ready
    ? 'opacity-100'
    : 'opacity-0'}"
>
  {#each group.metrics as m (m.value)}
    {#if m.value !== exclude}
      <div
        role="menuitem"
        tabindex="-1"
        class="cursor-pointer rounded px-2 py-1 text-xs hover:bg-blue-600 hover:text-white"
        onclick={() => onPick(m.value)}
        onkeydown={(e) => pickKey(e, m.value)}
      >
        {m.label}
      </div>
    {/if}
  {/each}
</div>
