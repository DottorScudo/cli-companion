// Layout della griglia (gruppi, vista globale) più il workspace "a fuoco senza
// gruppo": un workspace senza terminali non ha un gruppo nel layout, ma lo si
// mostra comunque (es. per aprirne il Tasks Manager). Pannelli e barra del
// titolo seguono il layout *mostrato* (`shownLayout`): mai i terminali di un
// altro workspace.

import { createStore } from "../lib/store.ts";
import { EMPTY_LAYOUT, computePaneLayouts } from "../lib/layout.ts";
import type { LayoutState, PaneLayout } from "../lib/layout.ts";

export interface LayoutStoreState {
  layout: LayoutState;
  /**
   * Workspace mostrato senza avere un gruppo (o con un gruppo nato in secondo
   * piano): decade quando la selezione del layout cambia per una scelta.
   */
  focusedWorkspaceId: string | null;
}

export const INITIAL_LAYOUT_STATE: LayoutStoreState = { layout: EMPTY_LAYOUT, focusedWorkspaceId: null };

/**
 * La selezione (gruppo/vista) cambia per una scelta, non da sé. Da sé: il
 * gruppo attivo sparisce chiudendone l'ultimo tab, un gruppo si attiva perché
 * non ce n'era nessuno (tab in secondo piano), la vista globale rimasta vuota
 * torna ai gruppi.
 */
function selectionChosen(prev: LayoutState, next: LayoutState): boolean {
  const viewChosen =
    next.activeView !== prev.activeView && !(prev.activeView === "global" && next.global.order.length === 0);
  const workspaceChosen =
    next.activeWorkspaceId !== prev.activeWorkspaceId &&
    prev.activeWorkspaceId !== null &&
    hasGroup(next, prev.activeWorkspaceId);
  return viewChosen || workspaceChosen;
}

/**
 * Applica una transizione del layout. Stesso layout: stesso stato. Se la
 * selezione cambia per una scelta il focus "senza gruppo" decade; un cambio
 * che non la tocca (es. riordino) o che la sposta da sé (chiusure e aperture
 * in secondo piano) lo lascia, così la vista non salta a un altro workspace.
 */
export function applyLayout(s: LayoutStoreState, fn: (l: LayoutState) => LayoutState): LayoutStoreState {
  const layout = fn(s.layout);
  if (layout === s.layout) return s;
  return { layout, focusedWorkspaceId: selectionChosen(s.layout, layout) ? null : s.focusedWorkspaceId };
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

const shownLayoutCache = new WeakMap<LayoutState, { focusedWorkspaceId: string; layout: LayoutState }>();

/**
 * Layout mostrato: con un workspace a fuoco, il layout in cui il suo gruppo è
 * quello attivo (nessun gruppo: nessun pannello visibile; gruppo nato in
 * secondo piano: i suoi pannelli). Memoizzato: stesso stato, stessa istanza.
 */
export function shownLayout(s: LayoutStoreState): LayoutState {
  const { layout, focusedWorkspaceId } = s;
  if (focusedWorkspaceId === null) return layout;
  if (layout.activeView === "workspace" && layout.activeWorkspaceId === focusedWorkspaceId) return layout;
  const cached = shownLayoutCache.get(layout);
  if (cached?.focusedWorkspaceId === focusedWorkspaceId) return cached.layout;
  const shown: LayoutState = { ...layout, activeWorkspaceId: focusedWorkspaceId, activeView: "workspace" };
  shownLayoutCache.set(layout, { focusedWorkspaceId, layout: shown });
  return shown;
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

/** Layout dei pannelli della vista mostrata (vedi `shownLayout`), memoizzato. */
export function shownPaneLayouts(s: LayoutStoreState): Record<string, PaneLayout> {
  return paneLayoutsOf(shownLayout(s));
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
