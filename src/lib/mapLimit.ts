// Limitatore di concorrenza: al più `limit` attività attive insieme, le altre in
// coda. Funzione pura a zero dipendenze, importabile sia dall'app sia dai test
// `node --test`.

/** Esegue `task` appena c'è posto e ne restituisce l'esito (anche un rifiuto). */
export type Limiter = <T>(task: () => Promise<T>) => Promise<T>;

/**
 * Crea un limitatore: tutte le attività passate allo stesso limitatore, da
 * chiamate anche sovrapposte, non superano mai `limit` (almeno 1) attive.
 */
export function createLimiter(limit: number): Limiter {
  const max = Math.max(1, limit);
  const waiting: Array<() => void> = [];
  let active = 0;

  const release = (): void => {
    const next = waiting.shift();
    if (next) next();
    else active -= 1;
  };

  return async <T>(task: () => Promise<T>): Promise<T> => {
    if (active < max) active += 1;
    else await new Promise<void>((resolve) => waiting.push(resolve));
    try {
      return await task();
    } finally {
      release();
    }
  };
}
