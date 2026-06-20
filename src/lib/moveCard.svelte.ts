// Shared state + loader for the move hover-card. A single host (`MoveHoverHost`) renders the card;
// any move row/list item just calls `showMoveCard`/`hideMoveCard` on hover. Details are fetched
// lazily from the backend and cached, so re-hovering a move is instant.
import { invoke } from "@tauri-apps/api/core";
import type { MoveCard } from "$lib/bindings";

const cache = new Map<number, MoveCard>();

export const moveCard = $state<{
  open: boolean;
  moveId: number | null;
  detail: MoveCard | null;
  // anchor rect of the hovered element, in viewport coords
  rect: { left: number; right: number; top: number; bottom: number } | null;
}>({ open: false, moveId: null, detail: null, rect: null });

export async function showMoveCard(moveId: number, el: HTMLElement) {
  const r = el.getBoundingClientRect();
  moveCard.open = true;
  moveCard.moveId = moveId;
  moveCard.rect = {
    left: r.left,
    right: r.right,
    top: r.top,
    bottom: r.bottom,
  };

  const cached = cache.get(moveId);
  if (cached) {
    moveCard.detail = cached;
    return;
  }
  moveCard.detail = null;
  const detail = await invoke<MoveCard>("move_card", { moveId });
  cache.set(moveId, detail);
  // ignore if the pointer has since moved to another move (or left)
  if (moveCard.open && moveCard.moveId === moveId) moveCard.detail = detail;
}

export function hideMoveCard() {
  moveCard.open = false;
}
