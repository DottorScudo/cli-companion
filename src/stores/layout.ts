// Layout della griglia (gruppi, vista globale) più il workspace "a fuoco senza
// gruppo": un workspace senza terminali non ha un gruppo nel layout, ma lo si
// mostra comunque (es. per aprirne il Tasks Manager).

import { createStore } from "../lib/store.ts";
import { EMPTY_LAYOUT, computePaneLayouts } from "../lib/layout.ts";
import type { LayoutState, PaneLayout } from "../lib/layout.ts";

export interface LayoutStoreState {
  layout: LayoutState;
  /** Workspace mostrato senza avere un gruppo: decade appena il layout cambia selezione. */
  focusedWorkspaceId: string | null;
}

export const INITIAL_LAYOUT_STATE: LayoutStoreState = { layout: EMPTY_LAYOUT, focusedWorkspaceId: null };

/**
 * Applica una transizione del layout. Stesso layout: stesso stato. Se cambia
 * il gruppo selezionato o la vista (gruppo/globale) il focus "senza gruppo"
 * decade; un cambio che non tocca la selezione (es. riordino) lo lascia.
 */
export function applyLayout(s: LayoutStoreState, fn: (l: LayoutState) => LayoutState): LayoutStoreState {
  const layout = fn(s.layout);
  if (layout === s.layout) return s;
  const selectionChanged =
    layout.activeWorkspaceId !== s.layout.activeWorkspaceId || layout.activeView !== s.layout.activeView;
  return { layout, focusedWorkspaceId: selectionChanged ? null : s.focusedWorkspaceId };
}

export function focusWorkspace(s: LayoutStoreState, id: string | null): LayoutStoreState {
  return s.focusedWorkspaceId === id ? s : { ...s, focusedWorkspaceId: id };
}

/** Workspace mostrato: quello a fuoco senza gruppo, altrimenti il gruppo attivo (null con la vista globale). */
export function viewWorkspaceId(s: LayoutStoreState): string | null {
  return s.focusedWorkspaceId ?? (s.layout.activeView === "global" ? null : s.layout.activeWorkspaceId);
}

export function hasGroup(layout: LayoutState, workspaceId: string): boolean {
  return layout.groups.some((g) => g.workspaceId === workspaceId);
}

const paneLayoutsCache = new WeakMap<LayoutState, Record<string, PaneLayout>>();

/** `computePaneLayouts` memoizzato per oggetto layout: più lettori condividono lo stesso risultato. */
export function paneLayoutsOf(layout: LayoutState): Record<string, PaneLayout> {
  let panes = paneLayoutsCache.get(layout);
  if (!panes) {
    panes = computePaneLayouts(layout);
    paneLayoutsCache.set(layout, panes);
  }
  return panes;
}

/** Stesso pannello disposto allo stesso modo? (per ridisegnare un pannello solo se cambia il suo layout) */
export function samePaneLayout(a: PaneLayout | undefined, b: PaneLayout | undefined): boolean {
  if (a === b) return true;
  if (!a || !b) return false;
  const sameRect =
    a.rect === b.rect ||
    (a.rect !== null &&
      b.rect !== null &&
      a.rect.left === b.rect.left &&
      a.rect.top === b.rect.top &&
      a.rect.width === b.rect.width &&
      a.rect.height === b.rect.height);
  return (
    sameRect &&
    a.visible === b.visible &&
    a.focused === b.focused &&
    a.maximized === b.maximized &&
    a.groupWorkspaceId === b.groupWorkspaceId &&
    a.pinned === b.pinned
  );
}

export function createLayoutStore() {
  const store = createStore<LayoutStoreState>(INITIAL_LAYOUT_STATE);
  return {
    ...store,
    apply: (fn: (l: LayoutState) => LayoutState) => store.update((s) => applyLayout(s, fn)),
    focusWorkspace: (id: string | null) => store.update((s) => focusWorkspace(s, id)),
  };
}

export const layoutStore = createLayoutStore();
