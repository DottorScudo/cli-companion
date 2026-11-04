// Code dei task: caricamento dal backend e memorizzazione nell'archivio.

import { api } from "../lib/tauri";
import type { BoardView } from "../lib/board";
import { boardsStore } from "../stores/boards";

/** Memorizza una board; una risposta più vecchia di quella nota (rev) non la sostituisce. */
export function storeBoard(board: BoardView): void {
  boardsStore.store(board);
}

/** Caricamento (o ricaricamento) di una board; l'errore si mostra solo nel Tasks Manager. */
export function loadBoard(workspaceId: string): void {
  api
    .boardGet(workspaceId)
    .then((board) => {
      storeBoard(board);
      boardsStore.clearError(workspaceId);
    })
    .catch((e) => boardsStore.setError(workspaceId, String(e)));
}
