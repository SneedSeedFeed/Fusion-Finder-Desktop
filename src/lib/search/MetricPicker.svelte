<script lang="ts">
  import { createDropdownMenu, melt } from "@melt-ui/svelte";
  import { fly } from "svelte/transition";
  import { METRIC_GROUPS, metricLabel, type Metric } from "$lib/bindings";
  import MetricFlyout from "$lib/search/MetricFlyout.svelte";

  // A cascade sort-metric picker. melt drives only the outer menu (open/close, outside-click,
  // Escape, trigger positioning); the *submenus are ours* — melt's createSubmenu gave no way to
  // hold a submenu open against pointer intent and got into stuck internal states when patched from
  // outside. Owning `openGroup` lets us apply a Floating-UI-style "safe polygon" cleanly.
  let {
    value = $bindable(),
    nullLabel,
    exclude = null,
    title,
  }: {
    value: Metric | null;
    nullLabel: string;
    exclude?: Metric | null;
    title?: string;
  } = $props();

  const {
    elements: { trigger, menu },
    states: { open },
  } = createDropdownMenu({
    forceVisible: true,
    positioning: { placement: "bottom-start" },
    loop: true,
  });

  const currentLabel = $derived(value === null ? nullLabel : metricLabel(value));

  let openGroup = $state<number | null>(null); // which category's submenu is showing
  let menuEl = $state<HTMLElement>();
  const rowEls: HTMLElement[] = [];

  function pick(m: Metric | null) {
    value = m;
    open.set(false);
  }

  // --- safe-polygon hover controller -------------------------------------------------------------
  type Pt = { x: number; y: number };
  const within = (r: DOMRect | undefined, p: Pt) =>
    !!r && p.x >= r.left && p.x <= r.right && p.y >= r.top && p.y <= r.bottom;
  const edge = (p: Pt, a: Pt, b: Pt) =>
    (p.x - b.x) * (a.y - b.y) - (a.x - b.x) * (p.y - b.y);
  function inTriangle(p: Pt, a: Pt, b: Pt, c: Pt) {
    const d1 = edge(p, a, b);
    const d2 = edge(p, b, c);
    const d3 = edge(p, c, a);
    return !((d1 < 0 || d2 < 0 || d3 < 0) && (d1 > 0 || d2 > 0 || d3 > 0));
  }

  // While the menu is open, a single pointer-move controller decides which submenu is open: hovering
  // a category opens it, but while the cursor is inside the triangle aimed at the *current* submenu
  // a stray hover over another row is ignored (the diagonal-overshoot fix).
  $effect(() => {
    if (!$open) {
      openGroup = null;
      return;
    }
    let prev: Pt = { x: 0, y: 0 };
    let apex: Pt | null = null;

    function onMove(e: PointerEvent) {
      const p = { x: e.clientX, y: e.clientY };
      const flyout =
        openGroup !== null
          ? menuEl?.querySelector<HTMLElement>(`[data-submenu="${openGroup}"]`)
          : null;
      const subRect = flyout?.getBoundingClientRect();

      // inside the open submenu -> keep it
      if (within(subRect, p)) {
        apex = null;
        prev = p;
        return;
      }

      const rowEl = (document.elementFromPoint(p.x, p.y) as HTMLElement | null)?.closest<HTMLElement>(
        "[data-cat-row]",
      );
      const overIdx = rowEl ? Number(rowEl.dataset.catRow) : null;

      // over the active category's own row -> keep
      if (openGroup !== null && overIdx === openGroup) {
        apex = null;
        prev = p;
        return;
      }

      // still aimed at the open submenu (inside the safe triangle) -> ignore the stray hover
      if (openGroup !== null && subRect) {
        if (!apex) apex = prev;
        if (
          inTriangle(
            p,
            apex,
            { x: subRect.left, y: subRect.top },
            { x: subRect.left, y: subRect.bottom },
          )
        ) {
          prev = p;
          return;
        }
      }

      // otherwise honour the hovered row (a number opens it, null closes the submenu)
      apex = null;
      openGroup = overIdx;
      prev = p;
    }

    window.addEventListener("pointermove", onMove, true);
    return () => window.removeEventListener("pointermove", onMove, true);
  });

  const rowClass =
    "flex cursor-pointer items-center justify-between rounded px-2 py-1 text-xs hover:bg-blue-600 hover:text-white";
</script>

<!-- fixed width so the toolbar layout and menu anchor don't jump as the selected label changes -->
<button
  type="button"
  use:melt={$trigger}
  {title}
  class="flex w-40 items-center justify-between gap-1 rounded border border-gray-700 bg-gray-800 px-2 py-1 text-xs text-gray-200 hover:bg-gray-700"
>
  <span class="truncate">{currentLabel}</span>
  <span class="shrink-0 text-gray-500">▾</span>
</button>

{#if $open}
  <div
    bind:this={menuEl}
    use:melt={$menu}
    class="z-30 w-40 rounded border border-gray-700 bg-gray-800 p-1 text-gray-200 shadow-lg focus:outline-none"
    transition:fly={{ duration: 100, y: -4 }}
  >
    <div
      role="menuitem"
      tabindex="-1"
      class={rowClass}
      onclick={() => pick(null)}
      onkeydown={(e) => (e.key === "Enter" || e.key === " ") && pick(null)}
    >
      {nullLabel}
    </div>
    <div class="my-1 h-px bg-gray-700"></div>
    {#each METRIC_GROUPS as group, i (group.label)}
      <div
        bind:this={rowEls[i]}
        data-cat-row={i}
        role="menuitem"
        tabindex="-1"
        class="{rowClass} {openGroup === i ? 'bg-blue-600 text-white' : ''}"
      >
        <span>{group.label}</span>
        <span class="text-gray-500">▸</span>
      </div>
    {/each}
    {#if openGroup !== null}
      <MetricFlyout
        reference={rowEls[openGroup]}
        group={METRIC_GROUPS[openGroup]}
        index={openGroup}
        {exclude}
        onPick={pick}
      />
    {/if}
  </div>
{/if}
