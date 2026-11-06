// Stato dell'Agents HUB: impostazioni, segretari svegli, canale e thread
// selezionati, riassunti dei thread per canale e conversazione aperta. Il mondo attivo (Terminali o
// Agents HUB) sta in `stores/ui.ts`. Ogni transizione ritorna lo
// stesso stato se non cambia nulla.

import { browserStorage } from "../lib/browserStorage.ts";
import { GENERAL_CHANNEL, appendMessage, removeSummary, upsertSummary, withoutThread } from "../lib/hub.ts";
import { previewOf, withPreview } from "../lib/hubChats.ts";
import type { ThreadPreview } from "../lib/hubChats.ts";
import { secretaryNames } from "../lib/hubSecretaries.ts";
import { baselineSeen, markSeen, pruneSeen, readSeen } from "../lib/hubUnread.ts";
import type { SeenMap } from "../lib/hubUnread.ts";
import { DEFAULT_HUB_VIEW_MODE, readViewMode } from "../lib/hubViewMode.ts";
import type { HubViewMode } from "../lib/hubViewMode.ts";
import { DEFAULT_THREAD_PANEL_SHOWN, readThreadPanelShown } from "../lib/hubThreadPanelPref.ts";
import type { ThreadFilter } from "../lib/hub.ts";
import { createStore } from "../lib/store.ts";
import type { StorageLike } from "../lib/taskPanelPrefs.ts";
import { persistHubPrefs } from "./hubPrefs.ts";
import type { HubMessage } from "../bindings/HubMessage.ts";
import type { HubSecretaryStatus } from "../bindings/HubSecretaryStatus.ts";
import type { HubSettingsView } from "../bindings/HubSettingsView.ts";
import type { HubThread } from "../bindings/HubThread.ts";
import type { HubThreadSummary } from "../bindings/HubThreadSummary.ts";

export interface HubState {
  /** Interruttore globale (`hub_settings.json`). Spento: nessuna traccia nell'interfaccia. */
  enabled: boolean;
  /** Impostazioni lette dal backend (senza la API key, solo se c'è); `null` finché non arrivano. Restano anche a HUB spento. */
  settings: HubSettingsView | null;
  /** Segretari svegli adesso, per canale e thread (`hub-secretary-status`). */
  secretaryStatus: readonly HubSecretaryStatus[];
  /** Canale selezionato: "general" o l'id di un workspace. */
  channel: string;
  filter: ThreadFilter;
  /** Riassunti dei thread per canale, dal più recente; un canale assente non è ancora stato letto. */
  threads: Readonly<Record<string, readonly HubThreadSummary[]>>;
  threadId: string | null;
  /** Conversazione del thread selezionato, quando è stata letta. */
  thread: HubThread | null;
  /** Messaggio evento a cui sta rispondendo il compositore. */
  replyTo: string | null;
  /** Come si mostrano i thread: per canale o come messenger. Preferenza globale, salvata in localStorage. */
  viewMode: HubViewMode;
  /** Pannello task e terminali a destra del thread mostrato. Preferenza globale, salvata in localStorage. */
  threadPanelShown: boolean;
  /** Anteprima dell'ultimo messaggio per thread (solo vista messenger), calcolata nel client. */
  previews: Readonly<Record<string, ThreadPreview>>;
  /** Messaggi visti per thread (solo client, salvato in localStorage): una chat è non letta se ne ha di più. */
  lastSeen: SeenMap;
  /** Primo avvio, senza nulla di salvato: i thread che si leggono per la prima volta si considerano letti. */
  seenBaseline: boolean;
}

export const INITIAL_HUB: HubState = {
  enabled: false,
  settings: null,
  secretaryStatus: [],
  channel: GENERAL_CHANNEL,
  filter: "open",
  threads: {},
  threadId: null,
  thread: null,
  replyTo: null,
  viewMode: DEFAULT_HUB_VIEW_MODE,
  threadPanelShown: DEFAULT_THREAD_PANEL_SHOWN,
  previews: {},
  lastSeen: {},
  seenBaseline: false,
};

/** Impostazioni lette o salvate. Spegnere l'HUB scarta thread, anteprime e stato dei segretari, non le impostazioni né le preferenze. */
export function settingsLoaded(s: HubState, settings: HubSettingsView): HubState {
  if (!settings.enabled) {
    return {
      ...INITIAL_HUB,
      settings,
      viewMode: s.viewMode,
      threadPanelShown: s.threadPanelShown,
      lastSeen: s.lastSeen,
      seenBaseline: s.seenBaseline,
    };
  }
  return { ...s, enabled: true, settings };
}

/** Nomi dei segretari, riservati: non diventano mai il nome di una sessione dei terminali. */
export function reservedBotNames(s: HubState): string[] {
  return s.settings === null ? [] : secretaryNames(s.settings.secretaries);
}

/** Elenco completo dei segretari svegli (l'evento lo manda intero a ogni cambiamento). Con l'HUB spento si ignora. */
export function secretaryStatusLoaded(s: HubState, list: readonly HubSecretaryStatus[]): HubState {
  return s.enabled ? { ...s, secretaryStatus: list } : s;
}

/** Cambia canale: thread aperto e risposta in corso non hanno più senso. */
export function channelSelected(s: HubState, channel: string): HubState {
  if (s.channel === channel) return s;
  return { ...s, channel, threadId: null, thread: null, replyTo: null };
}

export function filterSet(s: HubState, filter: ThreadFilter): HubState {
  return s.filter === filter ? s : { ...s, filter };
}

/** Elenco di un canale appena letto. */
export function threadsLoaded(s: HubState, channel: string, list: readonly HubThreadSummary[]): HubState {
  const firstRead = s.threads[channel] === undefined;
  const lastSeen = s.seenBaseline && firstRead ? baselineSeen(s.lastSeen, list) : s.lastSeen;
  return { ...s, threads: { ...s.threads, [channel]: list }, lastSeen };
}

/** Un thread è cambiato (`hub-update`, o creato qui): aggiorna la riga del suo canale. */
export function summaryUpdated(s: HubState, summary: HubThreadSummary): HubState {
  const current = s.threads[summary.channel] ?? [];
  const next = upsertSummary(current, summary);
  return next === current ? s : { ...s, threads: { ...s.threads, [summary.channel]: next } };
}

/**
 * Un thread è stato eliminato (qui o `hub-thread-removed`): sparisce da elenco,
 * anteprime e messaggi visti e, se era aperto, la conversazione si chiude.
 */
export function threadRemoved(s: HubState, channel: string, threadId: string): HubState {
  const current = s.threads[channel];
  const list = current === undefined ? current : removeSummary(current, threadId);
  const previews = withoutThread(s.previews, threadId);
  const lastSeen = withoutThread(s.lastSeen, threadId);
  const open = s.channel === channel && s.threadId === threadId;
  if (list === current && previews === s.previews && lastSeen === s.lastSeen && !open) return s;
  const threads = list === undefined || list === current ? s.threads : { ...s.threads, [channel]: list };
  const next = { ...s, threads, previews, lastSeen };
  return open ? { ...next, threadId: null, thread: null, replyTo: null } : next;
}

/** Seleziona un thread (o nessuno); la conversazione si rilegge dal backend. */
export function threadSelected(s: HubState, threadId: string | null): HubState {
  if (s.threadId === threadId) return s;
  return { ...s, threadId, thread: null, replyTo: null };
}

/** Conversazione letta: vale solo se è ancora il thread selezionato e non è più vecchia di quella nota. */
export function threadLoaded(s: HubState, thread: HubThread): HubState {
  if (s.threadId !== thread.id) return s;
  if (s.thread && s.thread.updated_at > thread.updated_at) return s;
  return withOpenPreview({ ...s, thread });
}

/** Messaggio appena inviato: lo aggiunge alla conversazione aperta e chiude la risposta. */
export function messageAdded(s: HubState, threadId: string, message: HubMessage): HubState {
  if (s.thread === null || s.thread.id !== threadId) return s;
  const thread = appendMessage(s.thread, message);
  return thread === s.thread && s.replyTo === null ? s : withOpenPreview({ ...s, thread, replyTo: null });
}

/** Inizia (o annulla, con `null`) la risposta a un messaggio. */
export function replySet(s: HubState, messageId: string | null): HubState {
  return s.replyTo === messageId ? s : { ...s, replyTo: messageId };
}

/** Cambia la modalità di vista dei thread. */
export function viewModeSet(s: HubState, viewMode: HubViewMode): HubState {
  return s.viewMode === viewMode ? s : { ...s, viewMode };
}

/** Mostra o nasconde il pannello task e terminali del thread. */
export function threadPanelShownSet(s: HubState, shown: boolean): HubState {
  return s.threadPanelShown === shown ? s : { ...s, threadPanelShown: shown };
}

/** L'anteprima di un thread letto per la vista messenger; con l'HUB spento si ignora. */
export function previewLoaded(s: HubState, threadId: string, preview: ThreadPreview): HubState {
  if (!s.enabled) return s;
  const previews = withPreview(s.previews, threadId, preview);
  return previews === s.previews ? s : { ...s, previews };
}

/** Con la vista messenger, l'anteprima del thread aperto segue la conversazione in memoria, senza rileggerla. */
function withOpenPreview(s: HubState): HubState {
  if (s.viewMode !== "messenger" || s.thread === null) return s;
  return previewLoaded(s, s.thread.id, previewOf(s.thread));
}

/** L'utente ha visto `count` messaggi del thread. */
export function threadSeen(s: HubState, threadId: string, count: number): HubState {
  const lastSeen = markSeen(s.lastSeen, threadId, count);
  return lastSeen === s.lastSeen ? s : { ...s, lastSeen };
}

/** Toglie dai messaggi visti i thread che non esistono più (solo a canali tutti letti). */
export function seenPruned(s: HubState, channelIds: readonly string[]): HubState {
  const lastSeen = pruneSeen(s.lastSeen, s.threads, channelIds);
  return lastSeen === s.lastSeen ? s : { ...s, lastSeen };
}

/** L'archivio, con le preferenze lette da `storage` e salvate a ogni cambiamento. */
export function createHubStore(storage: StorageLike | null = browserStorage()) {
  const seen = readSeen(storage);
  const store = createStore<HubState>({
    ...INITIAL_HUB,
    viewMode: readViewMode(storage),
    threadPanelShown: readThreadPanelShown(storage),
    lastSeen: seen ?? {},
    seenBaseline: seen === null,
  });
  persistHubPrefs(store, storage);
  return {
    ...store,
    viewModeSet: (viewMode: HubViewMode) => store.update((s) => viewModeSet(s, viewMode)),
    threadPanelShownSet: (shown: boolean) => store.update((s) => threadPanelShownSet(s, shown)),
    previewLoaded: (threadId: string, preview: ThreadPreview) =>
      store.update((s) => previewLoaded(s, threadId, preview)),
    threadSeen: (threadId: string, count: number) => store.update((s) => threadSeen(s, threadId, count)),
    seenPruned: (channelIds: readonly string[]) => store.update((s) => seenPruned(s, channelIds)),
    settingsLoaded: (settings: HubSettingsView) => store.update((s) => settingsLoaded(s, settings)),
    secretaryStatusLoaded: (list: readonly HubSecretaryStatus[]) => store.update((s) => secretaryStatusLoaded(s, list)),
    channelSelected: (channel: string) => store.update((s) => channelSelected(s, channel)),
    filterSet: (filter: ThreadFilter) => store.update((s) => filterSet(s, filter)),
    threadsLoaded: (channel: string, list: readonly HubThreadSummary[]) =>
      store.update((s) => threadsLoaded(s, channel, list)),
    summaryUpdated: (summary: HubThreadSummary) => store.update((s) => summaryUpdated(s, summary)),
    threadRemoved: (channel: string, threadId: string) => store.update((s) => threadRemoved(s, channel, threadId)),
    threadSelected: (threadId: string | null) => store.update((s) => threadSelected(s, threadId)),
    threadLoaded: (thread: HubThread) => store.update((s) => threadLoaded(s, thread)),
    messageAdded: (threadId: string, message: HubMessage) => store.update((s) => messageAdded(s, threadId, message)),
    replySet: (messageId: string | null) => store.update((s) => replySet(s, messageId)),
  };
}

export const hubStore = createHubStore();
