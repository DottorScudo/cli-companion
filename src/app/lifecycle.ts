// Ciclo di vita dell'app: salvataggi dello scrollback registrati dai
// TerminalView e salvataggio finale prima dell'uscita.

import { report } from "../lib/errors";
import { CLOSE_FLUSH_TIMEOUT_MS } from "../lib/session";
import { sessionSaver } from "./sessionSaver";

type Flush = () => Promise<void>;

// Salvataggi dello scrollback registrati dai TerminalView.
const flushers = new Map<string, Flush>();

/** Registra (o rimuove, con `null`) il salvataggio dello scrollback di un tab. */
export function registerFlush(tabId: string, flush: Flush | null): void {
  if (flush) flushers.set(tabId, flush);
  else flushers.delete(tabId);
}

let closing = false;

/** L'app si sta chiudendo (o riavviando): i pty che escono non sono uscite dell'utente. */
export function isClosing(): boolean {
  return closing;
}

/**
 * Salva sessione e scrollback, con un tetto di tempo (`CLOSE_FLUSH_TIMEOUT_MS`)
 * così la chiusura non resta mai appesa. Chiamato alla chiusura della finestra
 * e anche prima di un riavvio (`relaunch` non passa da `onCloseRequested`).
 */
export async function flushBeforeExit(): Promise<void> {
  closing = true;
  const work = Promise.all([sessionSaver.flush(), ...[...flushers.values()].map((flush) => flush())]).catch((e) =>
    report("salvataggio alla chiusura", e),
  );
  await Promise.race([work, new Promise((resolve) => setTimeout(resolve, CLOSE_FLUSH_TIMEOUT_MS))]);
}
