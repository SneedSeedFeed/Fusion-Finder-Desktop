<script lang="ts">
  import { SORTS, type SortBy } from "$lib/bindings";

  // Result count + sort controls + the change-game button. Sort state is bound back to the page.
  let {
    count,
    searching,
    sortBy = $bindable(),
    sortDesc = $bindable(),
    version,
    onChangeGame,
  }: {
    count: number;
    searching: boolean;
    sortBy: SortBy;
    sortDesc: boolean;
    version: string;
    onChangeGame: () => void;
  } = $props();
</script>

<header class="flex items-center gap-2 border-b border-gray-800 px-3 py-2">
  <span
    ><strong class="text-gray-100">{count.toLocaleString()}</strong> fusions</span
  >
  {#if searching}<span class="text-gray-500">· searching…</span>{/if}
  <label class="ml-auto flex items-center gap-1 text-xs text-gray-400">
    Sort
    <select
      class="rounded border border-gray-700 bg-gray-800 p-1 text-gray-200"
      bind:value={sortBy}
    >
      {#each SORTS as s (s.value)}<option value={s.value}>{s.label}</option
        >{/each}
    </select>
  </label>
  <button
    type="button"
    class="rounded border border-gray-700 bg-gray-800 px-2 py-1 text-gray-200 hover:bg-gray-700"
    title={sortDesc ? "Descending" : "Ascending"}
    aria-label={sortDesc ? "Descending" : "Ascending"}
    onclick={() => (sortDesc = !sortDesc)}>{sortDesc ? "↓" : "↑"}</button
  >
  <button
    type="button"
    class="rounded border border-gray-700 bg-gray-800 px-2 py-1 text-xs text-gray-300 hover:bg-gray-700"
    title="Change game folder or version"
    onclick={onChangeGame}>{version} ⚙</button
  >
</header>
