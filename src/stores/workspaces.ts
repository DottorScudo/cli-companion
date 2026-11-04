// Elenco dei workspace e stato del suo caricamento.

import { createStore } from "../lib/store.ts";
import type { Workspace } from "../lib/tauri.ts";

export type WorkspacesStatus = "loading" | "ready" | "error";

export interface WorkspacesState {
  list: readonly Workspace[];
  status: WorkspacesStatus;
  /** Messaggio del caricamento fallito (solo con `status === "error"`). */
  error: string | null;
}

export const INITIAL_WORKSPACES: WorkspacesState = { list: [], status: "loading", error: null };

/** Caricamento riuscito. */
export function loaded(s: WorkspacesState, list: readonly Workspace[]): WorkspacesState {
  return { ...s, list, status: "ready", error: null };
}

/** Caricamento fallito: l'elenco resta com'è. */
export function failed(s: WorkspacesState, error: string): WorkspacesState {
  return { ...s, status: "error", error };
}

export function added(s: WorkspacesState, ws: Workspace): WorkspacesState {
  return { ...s, list: [...s.list, ws] };
}

/** Sostituisce il workspace con lo stesso id (rinomina, modifica degli avvii). Id sconosciuto: invariato. */
export function updated(s: WorkspacesState, ws: Workspace): WorkspacesState {
  const current = s.list.find((w) => w.id === ws.id);
  if (current === undefined || current === ws) return s;
  return { ...s, list: s.list.map((w) => (w.id === ws.id ? ws : w)) };
}

export function removed(s: WorkspacesState, id: string): WorkspacesState {
  return s.list.some((w) => w.id === id) ? { ...s, list: s.list.filter((w) => w.id !== id) } : s;
}

/** Nuovo ordine (o elenco completo) restituito dal backend. */
export function reordered(s: WorkspacesState, list: readonly Workspace[]): WorkspacesState {
  return s.list === list ? s : { ...s, list };
}

const visibleCache = new WeakMap<readonly Workspace[], readonly Workspace[]>();

/** Gli archiviati non compaiono in nessun elenco (restano solo nelle impostazioni). Riferimento stabile per elenco. */
export function visibleWorkspaces(s: WorkspacesState): readonly Workspace[] {
  let visible = visibleCache.get(s.list);
  if (!visible) {
    visible = s.list.filter((w) => !w.archived);
    visibleCache.set(s.list, visible);
  }
  return visible;
}

export function workspaceById(s: WorkspacesState, id: string): Workspace | undefined {
  return s.list.find((w) => w.id === id);
}

export function createWorkspacesStore() {
  const store = createStore<WorkspacesState>(INITIAL_WORKSPACES);
  return {
    ...store,
    loaded: (list: readonly Workspace[]) => store.update((s) => loaded(s, list)),
    failed: (error: string) => store.update((s) => failed(s, error)),
    added: (ws: Workspace) => store.update((s) => added(s, ws)),
    updated: (ws: Workspace) => store.update((s) => updated(s, ws)),
    removed: (id: string) => store.update((s) => removed(s, id)),
    reordered: (list: readonly Workspace[]) => store.update((s) => reordered(s, list)),
    get: (id: string) => workspaceById(store.getState(), id),
  };
}

export const workspacesStore = createWorkspacesStore();
