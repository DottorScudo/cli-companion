// Persistenza della sessione, parte pura: serializzazione, dipendenze del
// salvataggio, ripristino dei tab e salvataggio con debounce. L'hook e
// `app/sessionSaver.ts` si limitano a collegarla agli archivi e al backend.

import { assignRestoredNames } from "../lib/bots.ts";
import { openTabBackground } from "../lib/layout.ts";
import type { LayoutState } from "../lib/layout.ts";
import {
  SESSION_SAVE_MAX_MS,
  SESSION_SAVE_MS,
  debounceDelay,
  restoreCommand,
  serializeSession,
} from "../lib/session.ts";
import type { RestoreProfileLike, SessionData, SessionTab } from "../lib/session.ts";
import type { RuntimeState, TabRuntime } from "./tabRuntime.ts";
import type { TabEntry, TabsState } from "./tabs.ts";

/** I worker dei task non si salvano né si ripristinano. */
const persistable = (tabs: TabsState): TabEntry[] => tabs.filter((t) => !t.automated);

export function toSessionTab(tab: TabEntry, rt: TabRuntime | undefined, withTitle: boolean): SessionTab {
  return {
    tabId: tab.key,
    workspaceId: tab.workspaceId,
    launchId: tab.launchId,
    command: tab.command,
    title: withTitle ? rt?.title : undefined,
    agentId: rt?.agent?.agentId,
    agentSessionId: rt?.agent?.agentSessionId,
    botName: tab.bot,
  };
}

/**
 * JSON della sessione. Senza titoli è la chiave di confronto del salvataggio
 * (i titoli cambiano di continuo con lo spinner degli agenti); con i titoli è
 * ciò che si scrive su disco.
 */
export function buildSessionJson(
  tabs: TabsState,
  runtime: RuntimeState,
  layout: LayoutState,
  withTitles: boolean,
): string {
  return serializeSession({
    activeWorkspaceId: layout.activeWorkspaceId,
    activeView: layout.activeView,
    groups: layout.groups,
    global: layout.global,
    tabs: persistable(tabs).map((t) => toSessionTab(t, runtime[t.key], withTitles)),
  });
}

/** Ciò da cui dipende il contenuto salvato, senza i titoli: confrontabile senza serializzare. */
export interface SessionDeps {
  tabs: TabsState;
  layout: LayoutState;
  /** Per ogni tab salvabile "chiave:agente:sessione", unite: id che arrivano dagli eventi dell'agente. */
  agents: string;
}

export function sessionDeps(tabs: TabsState, runtime: RuntimeState, layout: LayoutState): SessionDeps {
  const agents = persistable(tabs)
    .map((t) => `${t.key}:${runtime[t.key]?.agent?.agentId ?? ""}:${runtime[t.key]?.agent?.agentSessionId ?? ""}`)
    .join("\n");
  return { tabs, layout, agents };
}

export function sameSessionDeps(a: SessionDeps, b: SessionDeps): boolean {
  return Object.is(a.tabs, b.tabs) && Object.is(a.layout, b.layout) && a.agents === b.agents;
}

/** Stato volatile di un tab ripristinato: titolo salvato e, se c'era una sessione dell'agente, quella da riprendere. */
function restoredRuntime(t: SessionTab): Partial<TabRuntime> {
  const init: Partial<TabRuntime> = {};
  if (t.title) init.title = t.title;
  if (t.agentSessionId) init.agent = { phase: "idle", agentSessionId: t.agentSessionId, agentId: t.agentId };
  return init;
}

/**
 * Tab (e stato volatile) da ripristinare. Stessi tabId di prima: così
 * combaciano lo scrollback salvato e `CLI_COMPANION_SESSION`. I nomi dei bot
 * si assegnano insieme a quelli dei tab già presenti (es. worker arrivati
 * prima del ripristino): chi c'è tiene il nome, un ripristinato in conflitto
 * ne riceve uno nuovo.
 */
export function restoredTabs(
  data: SessionData,
  present: TabsState,
  pool: readonly string[],
  profiles: RestoreProfileLike[],
  random?: () => number,
): { tabs: TabEntry[]; runtime: Record<string, Partial<TabRuntime>> } {
  const names = assignRestoredNames(
    [...present.map((t) => ({ tabId: t.key, botName: t.bot })), ...data.tabs],
    pool,
    random,
  );
  const runtime: Record<string, Partial<TabRuntime>> = {};
  const tabs = data.tabs.map((t): TabEntry => {
    runtime[t.tabId] = restoredRuntime(t);
    return {
      key: t.tabId,
      workspaceId: t.workspaceId,
      launchId: t.launchId,
      command: t.command,
      startCommand: restoreCommand(t, profiles),
      bot: names.get(t.tabId)!,
      restored: true,
    };
  });
  return { tabs, runtime };
}

/**
 * Layout ripristinato più i tab già presenti: si parte da quello salvato e i
 * tab dei gruppi attuali che non vi compaiono (worker arrivati prima del
 * ripristino) si aggiungono in secondo piano, senza cambiare selezione.
 */
export function mergeRestoredLayout(restored: LayoutState, current: LayoutState): LayoutState {
  let merged = restored;
  for (const group of current.groups) {
    for (const key of group.order) merged = openTabBackground(merged, group.workspaceId, key);
  }
  return merged;
}

// ---------------------------------------------------------------------------
// Salvataggio con debounce
// ---------------------------------------------------------------------------

/** Orologio iniettabile, per provare il debounce senza attese reali. */
export interface SaverClock {
  now(): number;
  setTimeout(fn: () => void, ms: number): unknown;
  clearTimeout(id: unknown): void;
}

const realClock: SaverClock = {
  now: () => Date.now(),
  setTimeout: (fn, ms) => setTimeout(fn, ms),
  clearTimeout: (id) => clearTimeout(id as ReturnType<typeof setTimeout>),
};

export interface SessionSaverOptions {
  /** JSON della sessione, con o senza titoli. */
  build(withTitles: boolean): string;
  save(json: string): Promise<void>;
  onError(e: unknown): void;
  waitMs?: number;
  maxWaitMs?: number;
  clock?: SaverClock;
}

export interface SessionSaver {
  /** Il salvataggio resta spento finché la sessione non è stata letta con successo (niente sovrascrittura dopo un errore di I/O). */
  enable(): void;
  readonly enabled: boolean;
  /** Le dipendenze sono cambiate: se il contenuto è già quello salvato non fa nulla, altrimenti rimanda il salvataggio. */
  changed(): void;
  /** Annulla l'attesa e salva subito, con i titoli. Spento: nessun effetto. Rifiuta se il salvataggio fallisce. */
  flush(): Promise<void>;
  dispose(): void;
}

/**
 * Salvatore della sessione. Debounce con attesa massima: le modifiche
 * frequenti (titoli OSC dello spinner) non rimandano il salvataggio oltre
 * `maxWaitMs`. La chiave di confronto non include i titoli: una variazione di
 * titolo da sola non fa salvare; il JSON scritto li include, aggiornati al
 * momento del salvataggio.
 */
export function createSessionSaver(opts: SessionSaverOptions): SessionSaver {
  const waitMs = opts.waitMs ?? SESSION_SAVE_MS;
  const maxWaitMs = opts.maxWaitMs ?? SESSION_SAVE_MAX_MS;
  const clock = opts.clock ?? realClock;
  let enabled = false;
  let savedKey: string | null = null;
  let pendingSince: number | null = null;
  let timer: unknown = undefined;

  const cancelPending = () => {
    clock.clearTimeout(timer);
    timer = undefined;
    pendingSince = null;
  };

  /** Scrive la sessione; `savedKey` torna `null` se fallisce, così il prossimo cambio riprova. */
  const write = async (key: string): Promise<void> => {
    savedKey = key;
    try {
      await opts.save(opts.build(true));
    } catch (e) {
      savedKey = null;
      throw e;
    }
  };

  return {
    enable: () => void (enabled = true),
    get enabled() {
      return enabled;
    },
    changed() {
      if (!enabled) return;
      const key = opts.build(false);
      if (key === savedKey) {
        cancelPending();
        return;
      }
      const now = clock.now();
      if (pendingSince === null) pendingSince = now;
      clock.clearTimeout(timer);
      timer = clock.setTimeout(
        () => {
          timer = undefined;
          pendingSince = null;
          write(key).catch(opts.onError);
        },
        debounceDelay(now, pendingSince, waitMs, maxWaitMs),
      );
    },
    async flush() {
      if (!enabled) return;
      cancelPending();
      await write(opts.build(false));
    },
    dispose: cancelPending,
  };
}
