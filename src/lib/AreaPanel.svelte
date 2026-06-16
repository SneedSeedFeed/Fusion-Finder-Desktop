<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { typeIcon, typeNameMap, typeIds } from "$lib/typeIcon";
  import { methodLabel } from "$lib/inspector/format";
  import type { AreaEncounter, NamedId } from "$lib/bindings";

  // A non-blocking floating panel: pick a route, see what's on it for the chosen difficulty mode.
  let {
    types,
    onClose,
    onInspect,
  }: {
    types: NamedId[];
    onClose: () => void;
    onInspect?: (f: { head: number; body: number }) => void;
  } = $props();

  const typeNames = $derived(typeNameMap(types));
  function typeName(id: number): string {
    return typeNames.get(id) ?? "";
  }

  let locations = $state<string[]>([]);
  let query = $state("");
  let selected = $state<string | null>(null);
  let encounters = $state<AreaEncounter[]>([]);
  let loadingEncounters = $state(false);
  // Classic and Remix are the game's two wild-encounter difficulty tables.
  let mode = $state<"Classic" | "Remix">("Classic");
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      locations = await invoke<string[]>("area_locations");
    } catch (e) {
      error = String(e);
    }
  });

  const visibleLocations = $derived(
    query.trim()
      ? locations.filter((l) =>
          l.toLowerCase().includes(query.trim().toLowerCase()),
        )
      : locations,
  );

  // rows for the selected route in the current mode (mode-exclusive ones plus the shared "Both")
  const shown = $derived(
    encounters.filter((e) => e.mode === mode || e.mode === "Both"),
  );

  async function select(location: string) {
    selected = location;
    loadingEncounters = true;
    encounters = [];
    try {
      encounters = await invoke<AreaEncounter[]>("area_encounters", {
        location,
      });
    } catch (e) {
      error = String(e);
    } finally {
      loadingEncounters = false;
    }
  }

  function chance(e: AreaEncounter): string {
    return e.method === "Static" ||
      e.method === "Gift" ||
      e.method === "Roaming"
      ? "—"
      : `${e.chance}%`;
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onClose()} />

<aside
  class="fixed top-16 right-4 z-40 flex max-h-[80vh] w-96 flex-col rounded-lg border border-gray-800 bg-gray-900 text-gray-200 shadow-xl"
  aria-label="Area encounters"
>
  <header class="flex items-center gap-2 border-b border-gray-800 px-3 py-2">
    <span class="font-semibold">🗺 Area encounters</span>
    <button
      class="ml-auto rounded border border-gray-700 bg-gray-800 px-2 py-0.5 text-sm hover:bg-gray-700"
      title="Close"
      aria-label="Close"
      onclick={onClose}>✕</button
    >
  </header>

  {#if error}
    <p class="p-3 text-red-400">{error}</p>
  {:else}
    <div class="border-b border-gray-800 p-3">
      <input
        type="text"
        bind:value={query}
        placeholder="Filter routes…"
        class="w-full rounded border border-gray-700 bg-gray-800 px-2 py-1 text-sm text-gray-200 placeholder-gray-500"
      />
      <ul class="mt-2 max-h-40 space-y-0.5 overflow-auto">
        {#each visibleLocations as loc (loc)}
          <li>
            <button
              class="w-full rounded px-2 py-1 text-left text-sm hover:bg-gray-800 {selected ===
              loc
                ? 'bg-gray-800 font-medium text-gray-100'
                : 'text-gray-300'}"
              onclick={() => select(loc)}>{loc}</button
            >
          </li>
        {:else}
          <li class="px-2 py-1 text-xs text-gray-500 italic">
            {locations.length ? "No matching routes" : "Loading routes…"}
          </li>
        {/each}
      </ul>
    </div>

    <div class="min-h-0 flex-1 overflow-auto p-3">
      {#if !selected}
        <p class="text-xs text-gray-500 italic">
          Pick a route to see what's on it.
        </p>
      {:else}
        <div class="mb-2 flex items-center gap-2">
          <span class="font-medium">{selected}</span>
          <div
            class="ml-auto flex overflow-hidden rounded border border-gray-700"
          >
            {#each ["Classic", "Remix"] as const as m (m)}
              <button
                class="px-2 py-0.5 text-xs {mode === m
                  ? m === 'Classic'
                    ? 'bg-amber-900/60 text-amber-200'
                    : 'bg-violet-900/60 text-violet-200'
                  : 'bg-gray-800 text-gray-400 hover:bg-gray-700'}"
                onclick={() => (mode = m)}>{m}</button
              >
            {/each}
          </div>
        </div>

        {#if loadingEncounters}
          <p class="text-xs text-gray-500">Loading…</p>
        {:else if shown.length}
          <table class="w-full text-left text-xs">
            <thead class="text-gray-400">
              <tr>
                <th class="font-medium">Pokémon</th>
                <th class="font-medium">How</th>
                <th class="font-medium whitespace-nowrap">Lv.</th>
                <th class="text-right font-medium">%</th>
              </tr>
            </thead>
            <tbody>
              {#each shown as e, ei (ei)}
                <tr
                  class="border-t border-gray-800 {onInspect
                    ? 'cursor-pointer hover:bg-gray-800/60'
                    : ''}"
                  onclick={() =>
                    onInspect?.({ head: e.species, body: e.species })}
                >
                  <td class="py-0.5 pr-2">
                    <span class="flex items-center gap-1">
                      <span>{e.name}</span>
                      {#each typeIds(e.types) as t, ti (ti)}
                        <img
                          src={typeIcon(typeName(t))}
                          alt={typeName(t)}
                          class="h-3.5"
                        />
                      {/each}
                    </span>
                  </td>
                  <td class="pr-2 text-gray-500">{methodLabel(e.method)}</td>
                  <td class="pr-2 tabular-nums whitespace-nowrap"
                    >{e.min_level === e.max_level
                      ? e.min_level
                      : `${e.min_level}–${e.max_level}`}</td
                  >
                  <td class="text-right tabular-nums">{chance(e)}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {:else}
          <p class="text-xs text-gray-500 italic">
            Nothing here in {mode} mode.
          </p>
        {/if}
      {/if}
    </div>
  {/if}
</aside>
