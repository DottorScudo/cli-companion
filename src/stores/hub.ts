// Stato dell'Agents HUB: interruttore, canale e thread selezionati, riassunti
// dei thread per canale e conversazione aperta. L'apertura della vista sta in
// `stores/ui.ts` (si esclude con l'Ufficio). Ogni transizione ritorna lo
// stesso stato se non cambia nulla.

import { GENERAL_CHANNEL, appendMessage, upsertSummary } from "../lib/hub.ts";
import type { ThreadFilter } from "../lib/hub.ts";
import { createStore } from "../lib/store.ts";
import type { HubMessage } from "../bindings/HubMessage.ts";
import type { HubThread } from "../bindings/HubThread.ts";
import type { HubThreadSummary } from "../bindings/HubThreadSummary.ts";

export interface HubState {
  /** Interruttore globale (`hub_settings.json`). Spento: nessuna traccia nell'interfaccia. */
  enabled: boolean;
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
}

export const INITIAL_HUB: HubState = {
  enabled: false,
  channel: GENERAL_CHANNEL,
  filter: "open",
  threads: {},
  threadId: null,
  thread: null,
  replyTo: null,
};

/** Impostazioni lette o salvate. Spegnere l'HUB scarta tutto ciò che era in memoria. */
export function settingsLoaded(s: HubState, enabled: boolean): HubState {
  if (s.enabled === enabled) return s;
  return enabled ? { ...s, enabled } : INITIAL_HUB;
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
  return { ...s, threads: { ...s.threads, [channel]: list } };
}

/** Un thread è cambiato (`hub-update`, o creato qui): aggiorna la riga del suo canale. */
export function summaryUpdated(s: HubState, summary: HubThreadSummary): HubState {
  const current = s.threads[summary.channel] ?? [];
  const next = upsertSummary(current, summary);
  return next === current ? s : { ...s, threads: { ...s.threads, [summary.channel]: next } };
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
  return { ...s, thread };
}

/** Messaggio appena inviato: lo aggiunge alla conversazione aperta e chiude la risposta. */
export function messageAdded(s: HubState, threadId: string, message: HubMessage): HubState {
  if (s.thread === null || s.thread.id !== threadId) return s;
  const thread = appendMessage(s.thread, message);
  return thread === s.thread && s.replyTo === null ? s : { ...s, thread, replyTo: null };
}

/** Inizia (o annulla, con `null`) la risposta a un messaggio. */
export function replySet(s: HubState, messageId: string | null): HubState {
  return s.replyTo === messageId ? s : { ...s, replyTo: messageId };
}

export function createHubStore() {
  const store = createStore<HubState>(INITIAL_HUB);
  return {
    ...store,
    settingsLoaded: (enabled: boolean) => store.update((s) => settingsLoaded(s, enabled)),
    channelSelected: (channel: string) => store.update((s) => channelSelected(s, channel)),
    filterSet: (filter: ThreadFilter) => store.update((s) => filterSet(s, filter)),
    threadsLoaded: (channel: string, list: readonly HubThreadSummary[]) =>
      store.update((s) => threadsLoaded(s, channel, list)),
    summaryUpdated: (summary: HubThreadSummary) => store.update((s) => summaryUpdated(s, summary)),
    threadSelected: (threadId: string | null) => store.update((s) => threadSelected(s, threadId)),
    threadLoaded: (thread: HubThread) => store.update((s) => threadLoaded(s, thread)),
    messageAdded: (threadId: string, message: HubMessage) => store.update((s) => messageAdded(s, threadId, message)),
    replySet: (messageId: string | null) => store.update((s) => replySet(s, messageId)),
  };
}

export const hubStore = createHubStore();
