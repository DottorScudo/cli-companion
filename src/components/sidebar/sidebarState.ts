// Stato dell'interfaccia della sidebar condiviso tra le righe: rinomina,
// conferme, operazione in corso, errore, riga espansa e trascinamento. Ogni
// transizione ritorna lo stesso stato se non cambia nulla; le righe leggono
// solo la propria fetta (`rowView`).

import { createStore } from "../../lib/store.ts";
import type { DropPosition } from "../../lib/workspaceOrder.ts";

export interface DropTarget {
  id: string;
  pos: DropPosition;
}

export interface SidebarState {
  editingId: string | null;
  editingName: string;
  confirmingId: string | null;
  archivingId: string | null;
  /** Workspace con un'operazione di rinomina/rimozione/archiviazione in corso. */
  busyId: string | null;
  /** Errore dell'ultima operazione, mostrato nell'intestazione della lista. */
  error: string | null;
  /** Workspace espanso a card: segue il gruppo attivo, ma si può selezionare anche uno senza terminali. */
  expandedId: string | null;
  /** Trascinamento per l'ordine: riga trascinata e posizione di rilascio. */
  dragId: string | null;
  dropTarget: DropTarget | null;
}

export const INITIAL_SIDEBAR: SidebarState = {
  editingId: null,
  editingName: "",
  confirmingId: null,
  archivingId: null,
  busyId: null,
  error: null,
  expandedId: null,
  dragId: null,
  dropTarget: null,
};

/** Applica la modifica; se nessun campo cambia ritorna lo stesso stato. */
function patch(s: SidebarState, changes: Partial<SidebarState>): SidebarState {
  const keys = Object.keys(changes) as (keyof SidebarState)[];
  return keys.every((k) => Object.is(s[k], changes[k])) ? s : { ...s, ...changes };
}

/** Avvia la rinomina: azzera errore e conferme di qualsiasi riga. */
export function startEdit(s: SidebarState, id: string, name: string): SidebarState {
  return patch(s, { error: null, confirmingId: null, archivingId: null, editingId: id, editingName: name });
}

export function setEditingName(s: SidebarState, name: string): SidebarState {
  return patch(s, { editingName: name });
}

export function stopEdit(s: SidebarState): SidebarState {
  return patch(s, { editingId: null });
}

/** Chiede conferma della rimozione (esclude quella dell'archiviazione). */
export function askRemove(s: SidebarState, id: string): SidebarState {
  return patch(s, { error: null, archivingId: null, confirmingId: id });
}

/** Chiede conferma dell'archiviazione (esclude quella della rimozione). */
export function askArchive(s: SidebarState, id: string): SidebarState {
  return patch(s, { error: null, confirmingId: null, archivingId: id });
}

export function cancelRemove(s: SidebarState): SidebarState {
  return patch(s, { confirmingId: null });
}

export function cancelArchive(s: SidebarState): SidebarState {
  return patch(s, { archivingId: null });
}

/** Inizia un'operazione sul workspace: errore azzerato, pulsanti disabilitati. */
export function beginBusy(s: SidebarState, id: string): SidebarState {
  return patch(s, { busyId: id, error: null });
}

export function endBusy(s: SidebarState): SidebarState {
  return patch(s, { busyId: null });
}

export function setError(s: SidebarState, error: string | null): SidebarState {
  return patch(s, { error });
}

export function expand(s: SidebarState, id: string): SidebarState {
  return patch(s, { expandedId: id });
}

export function startDrag(s: SidebarState, id: string): SidebarState {
  return patch(s, { dragId: id });
}

/** Posizione di rilascio sopra la riga `id`; stesso stato se è già quella. */
export function dragOver(s: SidebarState, id: string, pos: DropPosition): SidebarState {
  const d = s.dropTarget;
  return d && d.id === id && d.pos === pos ? s : patch(s, { dropTarget: { id, pos } });
}

export function endDrag(s: SidebarState): SidebarState {
  return patch(s, { dragId: null, dropTarget: null });
}

/** Ciò che una riga disegna: le altre righe non lo vedono cambiare. */
export interface RowView {
  editing: boolean;
  confirming: boolean;
  archiving: boolean;
  busy: boolean;
  expanded: boolean;
  dragging: boolean;
  /** Dove disegnare la barra di rilascio sulla riga (mai sulla riga trascinata). */
  drop: DropPosition | null;
}

export function rowView(s: SidebarState, id: string): RowView {
  return {
    editing: s.editingId === id,
    confirming: s.confirmingId === id,
    archiving: s.archivingId === id,
    busy: s.busyId === id,
    expanded: s.expandedId === id,
    dragging: s.dragId === id,
    drop: s.dragId !== null && s.dragId !== id && s.dropTarget?.id === id ? s.dropTarget.pos : null,
  };
}

export function createSidebarStore() {
  const store = createStore<SidebarState>(INITIAL_SIDEBAR);
  return {
    ...store,
    startEdit: (id: string, name: string) => store.update((s) => startEdit(s, id, name)),
    setEditingName: (name: string) => store.update((s) => setEditingName(s, name)),
    stopEdit: () => store.update(stopEdit),
    askRemove: (id: string) => store.update((s) => askRemove(s, id)),
    askArchive: (id: string) => store.update((s) => askArchive(s, id)),
    cancelRemove: () => store.update(cancelRemove),
    cancelArchive: () => store.update(cancelArchive),
    beginBusy: (id: string) => store.update((s) => beginBusy(s, id)),
    endBusy: () => store.update(endBusy),
    setError: (error: string | null) => store.update((s) => setError(s, error)),
    expand: (id: string) => store.update((s) => expand(s, id)),
    startDrag: (id: string) => store.update((s) => startDrag(s, id)),
    dragOver: (id: string, pos: DropPosition) => store.update((s) => dragOver(s, id, pos)),
    endDrag: () => store.update(endDrag),
  };
}

export const sidebarStore = createSidebarStore();
