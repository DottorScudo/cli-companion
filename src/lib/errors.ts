import { noticeStore } from "./notices.ts";

const UNKNOWN_ERROR = "errore sconosciuto";
/** Lunghezza massima del dettaglio mostrato all'utente. */
export const MAX_DETAIL_LENGTH = 300;

/** Testo leggibile di un errore qualsiasi (i comandi Tauri rifiutano con stringhe). */
export function errorMessage(err: unknown): string {
  let text: string;
  if (typeof err === "string") text = err;
  else if (err instanceof Error) text = err.message;
  else if (typeof err === "object" && err !== null && "message" in err) text = String(err.message);
  else text = err === undefined || err === null ? "" : String(err);
  text = text.trim();
  if (text === "") return UNKNOWN_ERROR;
  return text.length > MAX_DETAIL_LENGTH ? `${text.slice(0, MAX_DETAIL_LENGTH)}…` : text;
}

/** "Ambito: dettaglio", con l'iniziale dell'ambito maiuscola. */
export function failureText(scope: string, err: unknown): string {
  const head = scope.charAt(0).toUpperCase() + scope.slice(1);
  return `${head}: ${errorMessage(err)}`;
}

export interface ReportOptions {
  /** Oltre al log, mostra un avviso all'utente (per le operazioni che si aspetta). */
  notify?: boolean;
}

/** Segnala un errore: sempre nel log, in un avviso se `notify`. `scope` è l'operazione fallita, in italiano. */
export function report(scope: string, err: unknown, options: ReportOptions = {}): void {
  console.error(`[${scope}]`, err);
  if (options.notify === true) {
    noticeStore.push({ key: `error:${scope}`, level: "error", text: failureText(scope, err) });
  }
}

/**
 * Gestore per `.catch` di un errore davvero ignorabile: `reason` dice perché
 * lo è. L'errore resta visibile solo nel log di debug.
 */
export function ignoreError(reason: string): (err: unknown) => void {
  if (reason.trim() === "") throw new Error("ignoreError richiede il motivo per cui l'errore è ignorabile");
  return (err) => console.debug(`[ignorato] ${reason}`, err);
}
