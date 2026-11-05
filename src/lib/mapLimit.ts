// Esegue `task` su ogni elemento, con al più `limit` esecuzioni insieme. Funzione
// pura a zero dipendenze, importabile sia dall'app sia dai test `node --test`.

/** `task` non deve rifiutare: il primo rifiuto interromperebbe l'avvio degli altri. */
export async function mapLimit<T>(items: readonly T[], limit: number, task: (item: T) => Promise<void>): Promise<void> {
  let next = 0;
  const worker = async (): Promise<void> => {
    while (next < items.length) await task(items[next++]);
  };
  await Promise.all(Array.from({ length: Math.min(Math.max(1, limit), items.length) }, worker));
}
