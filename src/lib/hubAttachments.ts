// Allegati dei messaggi dell'Agents HUB (docs/HUB.MD, "Allegati"): limiti,
// ammissione dei file scelti, bozze del compositore, anteprima e testi.
// Funzioni pure a zero dipendenze, importabili sia dall'app sia dai test `node --test`.

import type { HubAttachment } from "../bindings/HubAttachment.ts";

/** Allegati di un messaggio (come il backend, `ATTACHMENTS_MAX` in `hub/attachments.rs`). */
export const ATTACHMENTS_MAX = 5;

/** Dimensione di un allegato in byte, 10 MB (come il backend, `ATTACHMENT_SIZE_MAX`). */
export const ATTACHMENT_SIZE_MAX = 10 * 1024 * 1024;

const BYTES_PER_UNIT = 1024;
const SIZE_UNITS = ["B", "KB", "MB", "GB"] as const;

/** Nome di un file incollato che non ne ha uno. */
const FALLBACK_NAME = "allegato";

/** Formati che il webview mostra in un `<img>` (niente SVG: un documento, non un'immagine semplice). */
const PREVIEW_MIMES: ReadonlySet<string> = new Set([
  "image/png",
  "image/jpeg",
  "image/gif",
  "image/webp",
  "image/bmp",
  "image/avif",
  "image/x-icon",
]);

/** Estensione da dare a un'immagine incollata senza nome. */
const EXTENSION_OF_MIME: Readonly<Record<string, string>> = {
  "image/png": "png",
  "image/jpeg": "jpg",
  "image/gif": "gif",
  "image/webp": "webp",
  "image/bmp": "bmp",
};

/** Il tipo del clipboard che indica del testo: con questo l'incolla resta testo. */
const PLAIN_TEXT_TYPE = "text/plain";

/** L'allegato ha un'anteprima come immagine? */
export function hasPreview(mime: string): boolean {
  return PREVIEW_MIMES.has(mime.toLowerCase());
}

/** «512 B», «1,5 KB», «10 MB»: una cifra decimale (virgola) sotto le dieci unità. */
export function formatSize(bytes: number): string {
  let value = Math.max(0, bytes);
  let unit = 0;
  while (value >= BYTES_PER_UNIT && unit < SIZE_UNITS.length - 1) {
    value /= BYTES_PER_UNIT;
    unit += 1;
  }
  const digits = unit === 0 || value >= 10 ? 0 : 1;
  const text = value.toLocaleString("it-IT", { maximumFractionDigits: digits });
  return `${text} ${SIZE_UNITS[unit]}`;
}

/** L'ultimo componente di un percorso Windows o POSIX (il nome mostrato prima del caricamento). */
export function baseName(path: string): string {
  const parts = path.split(/[\\/]/).filter((p) => p !== "");
  return parts.length === 0 ? path : parts[parts.length - 1];
}

/** Il nome con cui caricare un file senza percorso; un'immagine incollata può non averne uno. */
export function draftFileName(name: string, mime: string): string {
  const trimmed = name.trim();
  if (trimmed !== "") return trimmed;
  const extension = EXTENSION_OF_MIME[mime.toLowerCase()];
  return extension ? `${FALLBACK_NAME}.${extension}` : FALLBACK_NAME;
}

/** L'incolla porta degli allegati? Solo file senza testo: incollare del testo resta com'era. */
export function pasteAttaches(types: readonly string[], fileCount: number): boolean {
  return fileCount > 0 && !types.includes(PLAIN_TEXT_TYPE);
}

/** Un file proposto: `size` è `null` se non si conosce prima del caricamento (file scelto con il dialog). */
export interface Candidate {
  name: string;
  size: number | null;
}

export interface Admission<T> {
  accepted: T[];
  /** Perché qualche file è rimasto fuori; `null` se sono entrati tutti. */
  error: string | null;
}

/**
 * I file che possono entrare in un messaggio che ne ha già `current`: fuori
 * quelli oltre i 10 MB e quelli oltre il quinto allegato, con un messaggio per
 * l'utente. Il backend ricontrolla entrambi i limiti.
 */
export function admitFiles<T extends Candidate>(current: number, candidates: readonly T[]): Admission<T> {
  const errors: string[] = [];
  const small = candidates.filter((c) => {
    const fits = c.size === null || c.size <= ATTACHMENT_SIZE_MAX;
    if (!fits) errors.push(tooLargeText(c.name));
    return fits;
  });
  const room = Math.max(0, ATTACHMENTS_MAX - current);
  if (small.length > room) errors.push(tooManyText());
  return { accepted: small.slice(0, room), error: errors.length === 0 ? null : errors.join(" ") };
}

/** Come il messaggio del backend per un file troppo grande. */
export function tooLargeText(name: string): string {
  return `«${name}» supera i ${ATTACHMENT_SIZE_MAX / (BYTES_PER_UNIT * BYTES_PER_UNIT)} MB consentiti per un allegato.`;
}

export function tooManyText(): string {
  return `Un messaggio può avere al massimo ${ATTACHMENTS_MAX} allegati.`;
}

/** Perché Invia aspetta. */
export const UPLOADING_TEXT = "Aspetta che gli allegati finiscano di caricarsi.";

/** Un allegato nel compositore: in caricamento, poi pronto con i metadati dello staging. */
export interface AttachmentDraft {
  key: string;
  name: string;
  size: number | null;
  /** Blob URL della miniatura (immagine incollata o trascinata), `null` senza. */
  preview: string | null;
  /** I metadati dello staging; `null` finché il caricamento è in corso. */
  attachment: HubAttachment | null;
}

export function isUploading(draft: AttachmentDraft): boolean {
  return draft.attachment === null;
}

/** Gli id da mandare con il messaggio, nell'ordine dei chip. */
export function readyAttachmentIds(drafts: readonly AttachmentDraft[]): string[] {
  return drafts.flatMap((d) => (d.attachment ? [d.attachment.id] : []));
}

/** Invia è attivo: niente invio in corso, nessun caricamento in corso, testo o almeno un allegato. */
export function canSend(text: string, drafts: readonly AttachmentDraft[], sending: boolean): boolean {
  if (sending || drafts.some(isUploading)) return false;
  return text.trim() !== "" || drafts.length > 0;
}

/** La bozza `key` con i metadati arrivati dallo staging (nome e dimensione definitivi). */
export function draftStaged(
  drafts: readonly AttachmentDraft[],
  key: string,
  attachment: HubAttachment,
): AttachmentDraft[] {
  return drafts.map((d) => (d.key === key ? { ...d, name: attachment.name, size: attachment.size, attachment } : d));
}

/** «nota.pdf, 1,5 MB, in caricamento»: il nome accessibile di un chip. */
export function draftLabel(draft: AttachmentDraft): string {
  const parts = [draft.name];
  if (draft.size !== null) parts.push(formatSize(draft.size));
  if (isUploading(draft)) parts.push("in caricamento");
  return parts.join(", ");
}
