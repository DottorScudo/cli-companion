import { useMemo } from "react";
import type { DragEvent } from "react";
import { reorderWorkspaces } from "../../app/actions/workspaces";
import { dropOrder, dropPositionAt } from "../../lib/workspaceOrder";
import { visibleWorkspaces, workspacesStore } from "../../stores/workspaces";
import { sidebarStore } from "./sidebarState";

/** Rilascio: se l'ordine cambia lo salva (un errore compare nella lista). */
function handleDrop(): void {
  const { dragId, dropTarget } = sidebarStore.getState();
  if (dragId && dropTarget) {
    const ids = visibleWorkspaces(workspacesStore.getState()).map((w) => w.id);
    const next = dropOrder(ids, dragId, dropTarget.id, dropTarget.pos);
    if (next !== ids) {
      sidebarStore.setError(null);
      reorderWorkspaces(next).catch((e) => sidebarStore.setError(String(e)));
    }
  }
  sidebarStore.endDrag();
}

/** Gestori del trascinamento per riordinare la riga del workspace `id` (stabili finché `id` non cambia). */
export function useWorkspaceReorder(id: string) {
  return useMemo(
    () => ({
      draggable: true,
      onDragStart: (e: DragEvent<HTMLElement>) => {
        // Niente trascinamento partendo da campi di testo.
        if ((e.target as HTMLElement).closest("input")) return e.preventDefault();
        e.dataTransfer.effectAllowed = "move";
        e.dataTransfer.setData("text/plain", id);
        sidebarStore.startDrag(id);
      },
      onDragEnd: () => sidebarStore.endDrag(),
      onDragOver: (e: DragEvent<HTMLElement>) => {
        if (!sidebarStore.getState().dragId) return;
        e.preventDefault();
        e.dataTransfer.dropEffect = "move";
        const r = e.currentTarget.getBoundingClientRect();
        sidebarStore.dragOver(id, dropPositionAt(e.clientY, r.top, r.height));
      },
      onDrop: (e: DragEvent<HTMLElement>) => {
        if (!sidebarStore.getState().dragId) return;
        e.preventDefault();
        handleDrop();
      },
    }),
    [id],
  );
}
