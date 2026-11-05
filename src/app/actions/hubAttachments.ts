// Agents HUB, allegati dei messaggi (docs/HUB.MD, "Allegati"): scelta dei file,
// caricamento nello staging, anteprima e apertura con l'app predefinita.
// Qui l'I/O; le regole (limiti, bozze) stanno in `lib/hubAttachments.ts`.

import { open } from "@tauri-apps/plugin-dialog";
import { openPath } from "@tauri-apps/plugin-opener";
import { api } from "../../lib/tauri";
import type { HubAttachment } from "../../lib/tauri";

/** I file scelti con il dialog di sistema (percorsi assoluti); nessuno se l'utente annulla. */
export async function pickAttachmentPaths(): Promise<string[]> {
  const selected = await open({ multiple: true, directory: false, title: "Allega file" });
  if (selected === null) return [];
  return Array.isArray(selected) ? selected : [selected];
}

/** Carica nello staging un file scelto con il dialog. Rifiuta con l'errore del backend. */
export function stageAttachmentFile(path: string): Promise<HubAttachment> {
  return api.hubStageAttachmentFile(path);
}

/** Carica nello staging un file senza percorso (incollato o trascinato). Rifiuta con l'errore del backend. */
export async function stageAttachmentBlob(name: string, blob: Blob): Promise<HubAttachment> {
  const bytes = new Uint8Array(await blob.arrayBuffer());
  return api.hubStageAttachmentBytes(name, bytes);
}

/** Blob URL con i byte di un allegato di un messaggio: chi lo chiede lo revoca. Rifiuta se il file non c'è più. */
export async function attachmentPreviewUrl(
  channel: string,
  threadId: string,
  attachmentId: string,
  mime: string,
): Promise<string> {
  const bytes = await api.hubReadAttachment(channel, threadId, attachmentId);
  return URL.createObjectURL(new Blob([bytes], { type: mime }));
}

/** Il percorso assoluto di un allegato di un messaggio. Rifiuta se il file non c'è più. */
export function attachmentPath(channel: string, threadId: string, attachmentId: string): Promise<string> {
  return api.hubAttachmentPath(channel, threadId, attachmentId);
}

/** Apre un allegato con l'app predefinita del sistema. Rifiuta con l'errore dell'opener. */
export function openAttachmentPath(path: string): Promise<void> {
  return openPath(path);
}
