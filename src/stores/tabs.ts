// Identità dei tab: cambia solo quando se ne apre, chiude o ripristina uno.
// Lo stato volatile (titolo, agente, busy) sta in `tabRuntime.ts`, così un
// titolo OSC non ridisegna nulla che dipenda solo dall'elenco dei tab.

import { createStore } from "../lib/store.ts";
import { pickBotName } from "../lib/bots.ts";

export interface TabEntry {
  /** Id del tab = `CLI_COMPANION_SESSION`. */
  key: string;
  workspaceId: string;
  /** Avvio da cui è stato aperto il tab (assente = shell semplice). */
  launchId?: string;
  /** Agente dell'avvio integrato da cui è nato il tab (assente = shell o avvio personalizzato). */
  agentId?: string;
  /** Comando dell'avvio ("" = nessuno): è quello che si salva nella sessione. */
  command: string;
  /**
   * Comando iniettato all'apertura del terminale: uguale a `command`, tranne
   * nei tab ripristinati (ripresa della conversazione dell'agente).
   */
  startCommand: string;
  /** Nome del bot della sessione: assegnato sempre, mostrato solo con la funzione attiva. */
  bot: string;
  /** Tab ripristinato da una sessione precedente (mostra la cronologia salvata). */
  restored?: boolean;
  /** Worker di un task: mai salvato né ripristinato, niente scrollback. */
  automated?: boolean;
  /** Cartella di lavoro del worker. */
  cwd?: string;
}

/** Nell'ordine di creazione, che è l'ordine di render dei `TerminalView` (mai riordinato). */
export type TabsState = readonly TabEntry[];

export function findTab(s: TabsState, key: string): TabEntry | undefined {
  return s.find((t) => t.key === key);
}

/** Accoda un tab. Chiave già presente: stesso stato. */
export function addTab(s: TabsState, tab: TabEntry): TabsState {
  return findTab(s, tab.key) ? s : [...s, tab];
}

/** Toglie un tab. Chiave assente: stesso stato. */
export function removeTab(s: TabsState, key: string): TabsState {
  return findTab(s, key) ? s.filter((t) => t.key !== key) : s;
}

/** Accoda solo i tab con chiave nuova, senza riordinare né toccare quelli presenti. */
export function mergeTabs(s: TabsState, incoming: readonly TabEntry[]): TabsState {
  const known = new Set(s.map((t) => t.key));
  const fresh = incoming.filter((t) => {
    if (known.has(t.key)) return false;
    known.add(t.key);
    return true;
  });
  return fresh.length === 0 ? s : [...s, ...fresh];
}

/** Nome per un nuovo bot: unico rispetto ai bot dei tab presenti e diverso dai nomi riservati. */
export function nextBotName(
  s: TabsState,
  pool: readonly string[],
  random?: () => number,
  reserved: Iterable<string> = [],
): string {
  return pickBotName(
    pool,
    s.map((t) => t.bot),
    random,
    reserved,
  );
}

/**
 * Archivio dei tab. `open` è sincrono: due tab aperti di seguito vedono già
 * il precedente e non prendono lo stesso nome (prima serviva un ref aggiornato a mano).
 */
export function createTabsStore(random?: () => number) {
  const store = createStore<TabsState>([]);
  return {
    ...store,
    /** Apre un tab con nome bot unico rispetto ai tab presenti e ai nomi `reserved`; `null` se la chiave esiste già. */
    open(tab: Omit<TabEntry, "bot">, pool: readonly string[], reserved: Iterable<string> = []): TabEntry | null {
      const current = store.getState();
      if (findTab(current, tab.key)) return null;
      const entry: TabEntry = { ...tab, bot: nextBotName(current, pool, random, reserved) };
      store.set(addTab(current, entry));
      return entry;
    },
    remove: (key: string) => store.update((s) => removeTab(s, key)),
    merge: (incoming: readonly TabEntry[]) => store.update((s) => mergeTabs(s, incoming)),
    get: (key: string) => findTab(store.getState(), key),
  };
}

export const tabsStore = createTabsStore();
