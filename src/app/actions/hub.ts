// Agents HUB: interruttore, caricamento di canali, thread e conversazione, invio
// dei messaggi. Lo stato sta in `stores/hub.ts`; qui l'I/O col backend.

import { api } from "../../lib/tauri";
import type { HubThreadSummary } from "../../lib/tauri";
import { report } from "../../lib/errors";
import { summarizeThread, threadNeedsReload } from "../../lib/hub";
import type { ThreadFilter } from "../../lib/hub";
import { hubStore } from "../../stores/hub";
import { uiStore } from "../../stores/ui";

/** Interruttore letto all'avvio: con l'HUB spento il backend non crea né legge altro. */
export function loadHubSettings(): void {
  api
    .hubGetSettings()
    .then((s) => hubStore.settingsLoaded(s.enabled))
    .catch((e) => report("caricamento delle impostazioni dell'Agents HUB", e, { notify: true }));
}

/** Accende o spegne l'HUB. Spegnerlo chiude la vista. Rifiuta con l'errore del backend. */
export async function setHubEnabled(enabled: boolean): Promise<void> {
  const settings = await api.hubSetEnabled(enabled);
  hubStore.settingsLoaded(settings.enabled);
  if (!settings.enabled) uiStore.setHub(false);
}

/** Legge i riassunti dei thread di un canale. */
export function loadChannelThreads(channel: string): void {
  api
    .hubListThreads(channel)
    .then((list) => hubStore.threadsLoaded(channel, list))
    .catch((e) => report("caricamento dei thread dell'Agents HUB", e, { notify: true }));
}

/** Apertura della vista: l'elenco di ogni canale, per i segnalini di chi aspetta l'utente. */
export function loadAllChannels(channels: readonly string[]): void {
  for (const channel of channels) loadChannelThreads(channel);
}

/** Rilegge la conversazione del thread aperto. */
export function loadThread(channel: string, threadId: string): void {
  api
    .hubGetThread(channel, threadId)
    .then((thread) => hubStore.threadLoaded(thread))
    .catch((e) => report("caricamento della conversazione", e, { notify: true }));
}

export function selectChannel(channel: string): void {
  hubStore.channelSelected(channel);
  loadChannelThreads(channel);
}

export function selectThread(channel: string, threadId: string): void {
  hubStore.threadSelected(threadId);
  loadThread(channel, threadId);
}

export function setThreadFilter(filter: ThreadFilter): void {
  hubStore.filterSet(filter);
}

export function startReply(messageId: string | null): void {
  hubStore.replySet(messageId);
}

/** Crea un thread libero nel canale e lo apre. Rifiuta con l'errore del backend. */
export async function createThread(channel: string, title: string): Promise<void> {
  const thread = await api.hubCreateThread(channel, title);
  hubStore.summaryUpdated(summarizeThread(thread));
  hubStore.channelSelected(thread.channel);
  hubStore.threadSelected(thread.id);
  hubStore.threadLoaded(thread);
}

/** Invia un messaggio dell'utente, con l'eventuale messaggio a cui risponde. Rifiuta con l'errore del backend. */
export async function sendMessage(
  channel: string,
  threadId: string,
  body: string,
  replyTo: string | null,
): Promise<void> {
  const message = await api.hubSend(channel, threadId, body, replyTo);
  hubStore.messageAdded(threadId, message);
}

/** `hub-update`: aggiorna la riga del thread e rilegge la conversazione se è quella aperta e non è aggiornata. */
export function onHubUpdate(summary: HubThreadSummary): void {
  const state = hubStore.getState();
  if (!state.enabled) return;
  hubStore.summaryUpdated(summary);
  if (threadNeedsReload(state.thread, summary)) loadThread(summary.channel, summary.id);
}
