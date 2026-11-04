export type Unlisten = () => void;

/**
 * Avvia una sottoscrizione asincrona (tipicamente `listen` di Tauri) e
 * restituisce la funzione che la annulla. Annullare prima che la promessa si
 * risolva (cleanup di StrictMode, smontaggio veloce) disiscrive appena
 * possibile: nessun listener orfano. Annullare due volte è innocuo.
 * Un errore, anche sincrono, va a `onError` e la sottoscrizione resta inattiva.
 */
export function startSubscription(subscribe: () => Promise<Unlisten>, onError: (err: unknown) => void): Unlisten {
  let cancelled = false;
  let unlisten: Unlisten | undefined;
  let pending: Promise<Unlisten>;
  try {
    pending = subscribe();
  } catch (err) {
    onError(err);
    return () => undefined;
  }
  pending.then((u) => {
    if (cancelled) u();
    else unlisten = u;
  }, onError);
  return () => {
    cancelled = true;
    unlisten?.();
    unlisten = undefined;
  };
}

const noop = (): void => undefined;

/** Risultato di `startSubscriptions`. */
export interface Subscriptions {
  /** Annulla tutte le sottoscrizioni (anche quelle non ancora risolte). Due volte è innocuo. */
  unlisten: Unlisten;
  /** Si risolve quando ogni sottoscrizione si è conclusa, riuscita o fallita. Non rifiuta mai. */
  ready: Promise<void>;
}

/** Come `startSubscription`, più una promessa che si risolve a sottoscrizione conclusa. */
function startTracked(subscribe: () => Promise<Unlisten>, onError: (err: unknown) => void) {
  const calls: Promise<Unlisten>[] = [];
  const unlisten = startSubscription(() => {
    const pending = subscribe();
    calls.push(pending);
    return pending;
  }, onError);
  // Registrata dopo quella di `startSubscription` sulla stessa promessa: quando
  // `settled` si risolve, l'`unlisten` è già stato memorizzato (o eseguito).
  // Chiamata sincrona fallita: nessuna promessa, già conclusa.
  const settled = calls.length > 0 ? calls[0].then(noop, noop) : Promise.resolve();
  return { unlisten, settled };
}

/**
 * Avvia più sottoscrizioni insieme (tipicamente i `listen` di Tauri). Ogni
 * errore va a `onError` senza fermare le altre. `ready` dice quando tutte
 * hanno risposto: serve a non annunciare "pronto" al backend prima che gli
 * eventi che potrebbe inviare abbiano un ascoltatore.
 */
export function startSubscriptions(
  subscribes: Array<() => Promise<Unlisten>>,
  onError: (err: unknown) => void,
): Subscriptions {
  const started = subscribes.map((subscribe) => startTracked(subscribe, onError));
  return {
    unlisten: () => started.forEach((s) => s.unlisten()),
    ready: Promise.all(started.map((s) => s.settled)).then(noop),
  };
}
