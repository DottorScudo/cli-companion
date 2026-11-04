// Archivio di stato generico, osservabile con `useSyncExternalStore`. Logica
// pura a zero dipendenze: importabile sia dall'app sia dai test `node --test`
// (type-stripping), come src/lib/notices.ts.

export type Listener = () => void;

export interface Store<T> {
  getState(): T;
  /** Sostituisce lo stato; nessuna notifica se `Object.is(prev, next)`. */
  set(next: T): void;
  /** Applica un aggiornamento; nessuna notifica se ritorna lo stesso stato. */
  update(fn: (prev: T) => T): void;
  subscribe(listener: Listener): () => void;
}

/**
 * Crea un archivio con stato iniziale `initial`. `getState()` riflette subito
 * ogni modifica (niente copie "mirror" da tenere allineate). Gli ascoltatori
 * sono avvisati in modo sincrono, solo se lo stato cambia davvero.
 */
export function createStore<T>(initial: T): Store<T> {
  let state = initial;
  // Ogni iscrizione ha una propria voce: lo stesso listener iscritto due volte
  // si annulla una iscrizione alla volta.
  const entries = new Set<{ listener: Listener }>();

  const set = (next: T): void => {
    if (Object.is(state, next)) return;
    state = next;
    // Copia: un ascoltatore può disiscriversi (o iscriverne altri) durante la
    // notifica. Chi si è disiscritto nel frattempo non viene più chiamato.
    for (const entry of [...entries]) {
      if (entries.has(entry)) entry.listener();
    }
  };

  return {
    getState: () => state,
    set,
    update: (fn) => set(fn(state)),
    subscribe(listener) {
      const entry = { listener };
      entries.add(entry);
      return () => void entries.delete(entry);
    },
  };
}

const isPlainObject = (v: unknown): v is Record<string, unknown> =>
  typeof v === "object" && v !== null && !Array.isArray(v);

/** Uguaglianza superficiale: array elemento per elemento, oggetti chiave per chiave (`Object.is`). */
export function shallowEqual(a: unknown, b: unknown): boolean {
  if (Object.is(a, b)) return true;
  if (Array.isArray(a) && Array.isArray(b)) {
    return a.length === b.length && a.every((v, i) => Object.is(v, b[i]));
  }
  if (isPlainObject(a) && isPlainObject(b)) {
    const keys = Object.keys(a);
    return (
      keys.length === Object.keys(b).length &&
      keys.every((k) => Object.prototype.hasOwnProperty.call(b, k) && Object.is(a[k], b[k]))
    );
  }
  return false;
}
