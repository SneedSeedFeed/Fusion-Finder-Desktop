import { invoke } from "@tauri-apps/api/core";
import { SvelteSet } from "svelte/reactivity";

// Mirror of the backend `Favourite` (national-dex ids of the head and body, stable across games).
export interface Favourite {
  head_dex: number;
  body_dex: number;
}

const key = (headDex: number, bodyDex: number): string =>
  `${headDex}.${bodyDex}`;

// Reactive view of the user's favourited fusions. The backend owns the on-disk list (keyed by
// stable dex ids); this loads it once on startup, mirrors it as a reactive set so reads in markup
// stay live, and writes through to the backend on toggle.
export class Favourites {
  // keys are "headDex.bodyDex"
  #keys = new SvelteSet<string>();

  async load() {
    const list = await invoke<Favourite[]>("favourites");
    this.#keys.clear();
    for (const f of list) this.#keys.add(key(f.head_dex, f.body_dex));
  }

  has(headDex: number, bodyDex: number): boolean {
    return this.#keys.has(key(headDex, bodyDex));
  }

  get size(): number {
    return this.#keys.size;
  }

  // The favourited fusions as dex-id pairs. Reads the reactive set, so callers re-run on change.
  get entries(): Favourite[] {
    return [...this.#keys].map((k) => {
      const [head_dex, body_dex] = k.split(".").map(Number);
      return { head_dex, body_dex };
    });
  }

  // Flip a fusion's favourite state; the backend decides (and persists) the new value, which we
  // then mirror locally.
  async toggle(headDex: number, bodyDex: number): Promise<boolean> {
    const on = await invoke<boolean>("toggle_favourite", { headDex, bodyDex });
    const k = key(headDex, bodyDex);
    if (on) this.#keys.add(k);
    else this.#keys.delete(k);
    return on;
  }
}
