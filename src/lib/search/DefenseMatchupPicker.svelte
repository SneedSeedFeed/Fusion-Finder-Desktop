<script lang="ts">
  import type { NamedId } from "$lib/bindings";
  import { typeIcon } from "$lib/typeIcon";
  import { multiplierGlyph } from "$lib/inspector/format";

  // Per-type defensive matchup picker. Each type shows its Scarlet/Violet pill with a small badge
  // extension for the required effectiveness (quarter units, matching the backend). `value` maps
  // typeId -> quarters; a type with no constraint ("None") is simply absent from the map, and its
  // badge shows "–" so the control keeps the same footprint either way.
  //
  // The relationship runs [0× → ¼× → ½× → None → 1× → 2× → 4×]. Left-click steps toward immune
  // (wrapping past 0× round to 4×); right-click steps toward weak (wrapping past 4× round to 0×).
  let {
    types,
    value = $bindable(),
  }: { types: NamedId[]; value: Record<number, number> } = $props();

  // null marks the "None" (no-constraint) slot in the middle of the cycle
  const CYCLE: (number | null)[] = [0, 1, 2, null, 4, 8, 16];

  function step(id: number, dir: 1 | -1) {
    const cur = id in value ? value[id] : null;
    const next =
      CYCLE[(CYCLE.indexOf(cur) + dir + CYCLE.length) % CYCLE.length];
    const copy = { ...value };
    if (next === null) delete copy[id];
    else copy[id] = next;
    value = copy;
  }

  // badge label + colour by class of matchup, so the grid is scannable at a glance
  function badge(q: number | null): { label: string; cls: string } {
    if (q === null) return { label: "–", cls: "bg-gray-800 text-gray-500" };
    const label = `${multiplierGlyph(q)}×`;
    if (q > 4) return { label, cls: "bg-red-900 text-red-200" }; // weak (2× / 4×)
    if (q === 4) return { label, cls: "bg-gray-700 text-gray-200" }; // neutral (1×)
    if (q > 0) return { label, cls: "bg-green-900 text-green-200" }; // resist (½× / ¼×)
    return { label, cls: "bg-violet-900 text-violet-200" }; // immune (0×)
  }
</script>

<div class="flex flex-wrap gap-1.5">
  {#each types as t (t.id)}
    {@const q = t.id in value ? value[t.id] : null}
    {@const b = badge(q)}
    <button
      type="button"
      class="inline-flex h-5 items-center overflow-hidden rounded-full align-middle transition hover:brightness-110 {q ===
      null
        ? 'opacity-70'
        : ''}"
      title={t.name}
      aria-label={`${t.name}: ${q === null ? "no constraint" : b.label}`}
      onclick={() => step(t.id, -1)}
      oncontextmenu={(e) => {
        e.preventDefault();
        step(t.id, 1);
      }}
    >
      <img src={typeIcon(t.name)} alt={t.name} class="h-5 w-auto" />
      <span
        class="-ml-2.5 flex h-5 w-9 items-center justify-center rounded-r-full pl-2 text-xs leading-none font-bold {b.cls}"
      >
        {b.label}
      </span>
    </button>
  {/each}
</div>
