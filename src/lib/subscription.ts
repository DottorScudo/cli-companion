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
