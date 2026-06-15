<script lang="ts">
  import type { NamedId } from "$lib/bindings";
  import { typeIcon } from "$lib/typeIcon";

  // A grid of toggleable type icons. Used for both the "Types" filter (capped at 2) and the
  // "Defense" filter (uncapped). `value` is the bound list of selected type ids.
  let {
    types,
    value = $bindable(),
    max = Infinity,
  }: { types: NamedId[]; value: number[]; max?: number } = $props();

  function toggle(id: number) {
    if (value.includes(id)) {
      value = value.filter((t) => t !== id);
    } else if (value.length < max) {
      value = [...value, id];
    }
  }
</script>

<div class="flex flex-wrap gap-1.5">
  {#each types as t (t.id)}
    <button
      type="button"
      class="rounded-full p-px transition {value.includes(t.id)
        ? 'ring-2 ring-blue-400'
        : 'opacity-45 hover:opacity-100'}"
      title={t.name}
      aria-pressed={value.includes(t.id)}
      onclick={() => toggle(t.id)}
    >
      <img src={typeIcon(t.name)} alt={t.name} class="h-4 w-auto" />
    </button>
  {/each}
</div>
