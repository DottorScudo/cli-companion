/**
 * Sessioni PTY dei tab sopravvissute a un rimontaggio del loro `TerminalView`.
 *
 * Il backend accetta una sola sessione per tab (`DuplicateTab`). Un terminale
 * smontato e subito rimontato (doppio montaggio di `StrictMode` in dev, Fast
 * Refresh) faceva partire un secondo spawn mentre il primo era ancora in volo:
 * il secondo falliva con "il tab ha gia' una sessione" e il primo, ormai
 * scartato, chiudeva la sua shell, lasciando il tab senza terminale.
 *
 * Qui il rilascio non chiude subito la sessione: la chiusura e' rinviata
 * (`defer`) e un nuovo aggancio dello stesso tab nel frattempo la riprende
 * (stessa shell, stesse variabili preparate dal backend, comando di avvio
 * iniettato una volta sola), ricevendo l'output arrivato nel mezzo. Uno spawn
 * nuovo per un tab parte solo dopo che la chiusura della sessione precedente
 * dello stesso tab e' terminata. Logica pura: spawn, chiusura e rinvio sono
 * iniettati.
 */

/** Eventi tenuti per un tab sganciato in attesa di un nuovo aggancio. */
export const MAX_DETACHED_EVENTS = 1000;

export interface PtyKeeperDeps {
  /** Chiude la sessione `id` nel backend. */
  close: (id: number) => Promise<void>;
  /** Pianifica `fn` a breve (dopo un eventuale rimontaggio sincrono); restituisce l'annullamento. */
  defer: (fn: () => void) => () => void;
  /** Errore di una chiusura: non si ignora mai. */
  onError: (scope: string, err: unknown) => void;
}

/** Aggancio di un `TerminalView` alla sessione del suo tab. */
export interface SessionLease {
  /** Id della sessione, a spawn riuscito (rifiuta se lo spawn fallisce). */
  readonly session: Promise<number>;
  /** Sessione ripresa da un aggancio precedente (non creata ora). */
  readonly adopted: boolean;
  /** Vero solo la prima volta per sessione: il comando di avvio si inietta una volta. */
  claimLaunch: () => boolean;
  /** Sgancia: la sessione si chiude se nessuno la riprende prima del rinvio. Idempotente. */
  release: () => void;
}

/** Avvia la shell del tab: `deliver` riceve ogni evento della sessione. */
export type SpawnSession<E> = (deliver: (event: E) => void) => Promise<number>;

interface Entry<E> {
  session: Promise<number>;
  sink: ((event: E) => void) | null;
  buffer: E[];
  owner: symbol | null;
  launchClaimed: boolean;
  cancelClose: (() => void) | null;
}

export interface PtyKeeper<E> {
  /** Aggancia `onEvent` alla sessione del tab: la riprende se ancora aperta, altrimenti la crea con `spawn`. */
  attach: (tabId: string, onEvent: (event: E) => void, spawn: SpawnSession<E>) => SessionLease;
}

export function createPtyKeeper<E>(deps: PtyKeeperDeps): PtyKeeper<E> {
  const entries = new Map<string, Entry<E>>();
  // Chiusure in corso per tab: uno spawn nuovo dello stesso tab le aspetta.
  const closing = new Map<string, Promise<void>>();

  const deliver = (entry: Entry<E>, event: E) => {
    if (entry.sink) {
      entry.sink(event);
      return;
    }
    if (entry.buffer.length >= MAX_DETACHED_EVENTS) entry.buffer.shift();
    entry.buffer.push(event);
  };

  const closeEntry = (tabId: string, entry: Entry<E>) => {
    if (entries.get(tabId) === entry) entries.delete(tabId);
    const done = entry.session.then(
      (id) => deps.close(id).catch((e) => deps.onError("chiusura del terminale", e)),
      // Spawn fallito: niente da chiudere (l'errore l'ha gia' ricevuto chi agganciava).
      () => undefined,
    );
    closing.set(tabId, done);
    void done.then(() => {
      if (closing.get(tabId) === done) closing.delete(tabId);
    });
  };

  const create = (tabId: string, spawn: SpawnSession<E>): Entry<E> => {
    const previous = closing.get(tabId);
    const entry: Entry<E> = {
      session: Promise.resolve(0),
      sink: null,
      buffer: [],
      owner: null,
      launchClaimed: false,
      cancelClose: null,
    };
    const start = async () => spawn((event) => deliver(entry, event));
    // Senza chiusure in corso lo spawn parte subito.
    entry.session = previous ? previous.then(start) : start();
    // Uno spawn fallito libera il tab: il prossimo aggancio riprova.
    entry.session.catch(() => {
      if (entries.get(tabId) === entry) entries.delete(tabId);
    });
    entries.set(tabId, entry);
    return entry;
  };

  const attach = (tabId: string, onEvent: (event: E) => void, spawn: SpawnSession<E>): SessionLease => {
    const existing = entries.get(tabId);
    const adopted = existing !== undefined;
    const entry = existing ?? create(tabId, spawn);
    entry.cancelClose?.();
    entry.cancelClose = null;
    const owner = Symbol(tabId);
    entry.owner = owner;
    entry.sink = onEvent;
    const pending = entry.buffer;
    entry.buffer = [];
    for (const event of pending) onEvent(event);

    let released = false;
    return {
      session: entry.session,
      adopted,
      claimLaunch: () => {
        if (entry.owner !== owner || entry.launchClaimed) return false;
        entry.launchClaimed = true;
        return true;
      },
      release: () => {
        if (released) return;
        released = true;
        // Un aggancio piu' recente ha gia' preso la sessione: non e' piu' nostra.
        if (entry.owner !== owner) return;
        entry.owner = null;
        entry.sink = null;
        entry.cancelClose = deps.defer(() => {
          entry.cancelClose = null;
          closeEntry(tabId, entry);
        });
      },
    };
  };

  return { attach };
}
