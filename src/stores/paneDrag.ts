// Trascinamento delle intestazioni dei pannelli: origine e bersaglio
// evidenziato. L'origine sta nello stato, quindi anche gli handler la leggono
// da `getState()` senza ref.

import { createStore } from "../lib/store.ts";
import type { PaneLayout } from "../lib/layout.ts";

export interface PaneDragState {
  from: string | null;
  over: string | null;
}

export const INITIAL_DRAG: PaneDragState = { from: null, over: null };

export function startDrag(s: PaneDragState, id: string): PaneDragState {
  return s.from === id && s.over === null ? s : { from: id, over: null };
}

/**
 * Il puntatore è sopra `id`: diventa bersaglio solo se si sta trascinando un
 * altro pannello dello stesso gruppo (il riordino non esce dal gruppo).
 */
export function dragOver(
  s: PaneDragState,
  id: string,
  paneLayouts: Readonly<Record<string, Pick<PaneLayout, "groupWorkspaceId">>>,
): PaneDragState {
  const { from } = s;
  const allowed =
    from !== null && from !== id && paneLayouts[from]?.groupWorkspaceId === paneLayouts[id]?.groupWorkspaceId;
  const over = allowed ? id : null;
  return s.over === over ? s : { ...s, over };
}

/** Il puntatore lascia `id`: si spegne l'evidenziazione solo se era lui il bersaglio. */
export function leaveDrag(s: PaneDragState, id: string): PaneDragState {
  return s.over === id ? { ...s, over: null } : s;
}

export function endDrag(s: PaneDragState): PaneDragState {
  return s.from === null && s.over === null ? s : INITIAL_DRAG;
}

export function createPaneDragStore() {
  const store = createStore<PaneDragState>(INITIAL_DRAG);
  return {
    ...store,
    start: (id: string) => store.update((s) => startDrag(s, id)),
    dragOver: (id: string, paneLayouts: Readonly<Record<string, Pick<PaneLayout, "groupWorkspaceId">>>) =>
      store.update((s) => dragOver(s, id, paneLayouts)),
    leave: (id: string) => store.update((s) => leaveDrag(s, id)),
    end: () => store.update(endDrag),
  };
}

export const paneDragStore = createPaneDragStore();
