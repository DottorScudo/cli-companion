import { useEffect, useRef } from "react";
import { loadBoard, storeBoard } from "../app/boards";
import { notifyIfUnfocused, notifyPlainIfUnfocused } from "../lib/systemNotify";
import { workspacesStore } from "../stores/workspaces";
import { useStore } from "./useStore";
import { useTauriEvent } from "./useTauriEvent";

/**
 * Code dei task di tutti i workspace, nell'archivio `boardsStore`: caricamento
 * all'avvio (e per i workspace aggiunti dopo) e aggiornamenti dall'evento
 * `board-update`. Con il backend assente o in errore il riepilogo resta
 * vuoto; l'errore si mostra solo nel Tasks Manager.
 */
export function useBoardsSync() {
  const workspaces = useStore(workspacesStore, (s) => s.list);

  // Riepilogo delle board all'avvio e per i workspace aggiunti dopo.
  const requestedRef = useRef(new Set<string>());
  useEffect(() => {
    for (const w of workspaces) {
      if (requestedRef.current.has(w.id)) continue;
      requestedRef.current.add(w.id);
      loadBoard(w.id);
    }
  }, [workspaces]);

  // Aggiornamenti della board dal backend.
  useTauriEvent("board-update", storeBoard);
}

/**
 * Notifiche di sistema dei task (`task-notify`): un task richiede attenzione, ha
 * una domanda per l'utente o è fallito. Titolo = nome del workspace, testo = quello italiano del backend.
 * Con il tab del worker ancora aperto il toast è cliccabile (porta al tab);
 * solo a finestra non in primo piano, come le altre notifiche.
 */
export function useTaskNotify() {
  useTauriEvent("task-notify", (e) => {
    const name = workspacesStore.get(e.workspace_id)?.name ?? "Workspace";
    if (e.tab_id) void notifyIfUnfocused(e.tab_id, name, e.body);
    else void notifyPlainIfUnfocused(name, e.body);
  });
}
