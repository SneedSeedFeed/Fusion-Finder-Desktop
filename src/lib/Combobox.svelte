<script lang="ts">
  import { createCombobox, melt } from "@melt-ui/svelte";
  import { fly } from "svelte/transition";

  interface Item {
    id: number;
    name: string;
  }

  let {
    items,
    placeholder = "— any —",
    value = $bindable(),
  }: { items: Item[]; placeholder?: string; value?: number | null } = $props();

  const {
    elements: { menu, input, option },
    states: { open, inputValue, selected },
  } = createCombobox<number>({
    forceVisible: true,
    onSelectedChange: ({ next }) => {
      value = next?.value ?? null;
      return next;
    },
  });

  const filtered = $derived.by(() => {
    const q = $inputValue.trim().toLowerCase();
    return q ? items.filter((i) => i.name.toLowerCase().includes(q)) : items;
  });

  // melt doesn't sync the input text to the selection on its own — when the menu closes,
  // snap it back to the selected label (or empty), discarding any half-typed query.
  $effect(() => {
    if (!$open) {
      inputValue.set($selected?.label ?? "");
    }
  });

  function clear() {
    selected.set(undefined);
    inputValue.set("");
    value = null;
  }
</script>

<div class="relative">
  <input
    use:melt={$input}
    {placeholder}
    class="w-full rounded border border-gray-300 p-1 pr-6"
  />
  {#if value !== null && value !== undefined}
    <button
      type="button"
      onclick={clear}
      aria-label="clear"
      class="absolute top-1/2 right-1 -translate-y-1/2 px-1 text-gray-400 hover:text-gray-700"
    >×</button>
  {/if}

  {#if $open}
    <ul
      use:melt={$menu}
      transition:fly={{ duration: 100, y: -4 }}
      class="z-10 flex max-h-60 flex-col overflow-y-auto rounded border border-gray-200 bg-white shadow-lg"
    >
      {#each filtered as item (item.id)}
        <li
          use:melt={$option({ value: item.id, label: item.name })}
          class="cursor-pointer px-2 py-1 text-sm data-[highlighted]:bg-blue-100 data-[selected]:font-semibold"
        >
          {item.name}
        </li>
      {:else}
        <li class="px-2 py-1 text-sm text-gray-400">no matches</li>
      {/each}
    </ul>
  {/if}
</div>
