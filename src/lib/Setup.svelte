<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";

  type GameVersion = "Kanto" | "Hoenn";
  interface GameConfig {
    dir: string;
    version: GameVersion;
  }

  // onReady fires once a game has loaded; onCancel (optional) backs out of a re-run setup.
  let {
    onReady,
    onCancel,
  }: { onReady: (config: GameConfig) => void; onCancel?: () => void } =
    $props();

  let dir = $state<string | null>(null);
  let detected = $state<GameVersion | null>(null);
  let version = $state<GameVersion>("Kanto");
  let loading = $state(false);
  let error = $state<string | null>(null);

  const VERSIONS: { value: GameVersion; label: string }[] = [
    { value: "Kanto", label: "Infinite Fusion (Kanto)" },
    { value: "Hoenn", label: "Infinite Fusion: Hoenn" },
  ];

  async function browse() {
    error = null;
    const picked = await open({
      directory: true,
      title: "Select your Infinite Fusion folder",
    });
    if (typeof picked !== "string") return; // cancelled
    dir = picked;
    detected = await invoke<GameVersion | null>("detect_game", { dir: picked });
    if (detected) version = detected;
  }

  async function loadGame() {
    if (!dir) return;
    loading = true;
    error = null;
    try {
      const config = await invoke<GameConfig>("load_game", { dir, version });
      onReady(config);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }
</script>

<div
  class="flex h-screen w-full items-center justify-center bg-[#0d1117] p-6 text-gray-200"
>
  <div
    class="w-full max-w-md rounded-xl border border-gray-800 bg-gray-900/60 p-6 shadow-xl"
  >
    <h1 class="mb-1 text-xl font-semibold text-gray-100">Fusion Finder</h1>
    <p class="mb-5 text-sm text-gray-400">
      Point Fusion Finder at your copy of Pokémon Infinite Fusion to get
      started.
    </p>

    <button
      type="button"
      class="mb-2 w-full rounded-lg border border-gray-700 bg-gray-800 px-3 py-2 text-left text-sm hover:bg-gray-700"
      onclick={browse}
    >
      {#if dir}
        <span class="block truncate font-mono text-xs text-gray-300" title={dir}
          >{dir}</span
        >
        <span class="text-xs text-gray-500">Choose a different folder…</span>
      {:else}
        Browse for game folder…
      {/if}
    </button>

    {#if dir && detected === null}
      <p class="mb-2 text-xs text-amber-400">
        This doesn't look like an Infinite Fusion folder. Pick the version
        manually if you're sure.
      </p>
    {/if}

    {#if dir}
      <fieldset class="mb-4 mt-3 rounded-md border border-gray-800 p-2">
        <legend class="px-1 text-sm font-semibold text-gray-300">Version</legend
        >
        {#each VERSIONS as v (v.value)}
          <label class="flex items-center gap-2 px-1 py-1 text-sm">
            <input
              type="radio"
              class="accent-blue-500"
              name="version"
              value={v.value}
              bind:group={version}
            />
            {v.label}
            {#if detected === v.value}<span class="text-xs text-blue-400"
                >(detected)</span
              >{/if}
          </label>
        {/each}
      </fieldset>
    {/if}

    {#if error}
      <p class="mb-3 text-sm text-red-400">Couldn't load game data: {error}</p>
    {/if}

    <div class="flex gap-2">
      <button
        type="button"
        class="flex-1 rounded-lg bg-blue-600 px-3 py-2 text-sm font-semibold text-white hover:bg-blue-500 disabled:opacity-50"
        disabled={!dir || loading}
        onclick={loadGame}
      >
        {loading ? "Loading…" : "Load game"}
      </button>
      {#if onCancel}
        <button
          type="button"
          class="rounded-lg border border-gray-700 bg-gray-800 px-3 py-2 text-sm text-gray-300 hover:bg-gray-700"
          disabled={loading}
          onclick={onCancel}>Cancel</button
        >
      {/if}
    </div>
  </div>
</div>
