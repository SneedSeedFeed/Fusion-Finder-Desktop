import type { NamedId } from "$lib/bindings";

export function typeIcon(key: string): string {
  return `/types/${key.toUpperCase()}.png`;
}

export function categoryIcon(category: number): string {
  return `/moves/${category}.png`;
}

// id -> display name, the single source for resolving the type ids the backend sends. Build once
// from the bootstrap `types` table (e.g. `const names = $derived(typeNameMap(options.types))`).
export function typeNameMap(types: NamedId[]): Map<number, string> {
  return new Map(types.map((t) => [t.id, t.name]));
}

// Flatten a backend (primary, optional secondary) type pair into 1 or 2 ids for rendering.
export function typeIds(pair: readonly [number, number | null]): number[] {
  return pair[1] === null ? [pair[0]] : [pair[0], pair[1]];
}
