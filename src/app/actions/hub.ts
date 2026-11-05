// Agents HUB: interruttore, caricamento di canali, thread e conversazione, invio
// dei messaggi. Lo stato sta in `stores/hub.ts`; qui l'I/O col backend.

import { api } from "../../lib/tauri";
import type { HubSecretaryStatus, HubThreadSummary } from "../../lib/tauri";
import { report } from "../../lib/errors";
import { summarizeThread, threadNeedsReload } from "../../lib/hub";
import { threadAsksTask, threadsToSearch } from "../../lib/hubQuestions";
import type { ThreadFilter } from "../../lib/hub";
import { hubStore } from "../../stores/hub";
import { hubChannelIdsOf } from "../../stores/hubChannels";
import { uiStore } from "../../stores/ui";
import { workspacesStore } from "../../stores/workspaces";
import { markOpenChatSeen } from "./hubChatState";

/**
 * Impostazioni lette all'avvio (interruttore, modello, segretari): con l'HUB
 * spento il backend non crea né legge altro. Non rifiuta mai: l'errore è già
 * segnalato, e il ripristino della sessione può aspettarla.
 */
export function loadHubSettings(): Promise<void> {
  return api
    .hubGetSettings()
    .then((s) => hubStore.settingsLoaded(s))
    .catch((e) => report("caricamento delle impostazioni dell'Agents HUB", e, { notify: true }));
}

/** Segretari svegli adesso (con l'HUB acceso), poi li tiene aggiornati `hub-secretary-status`. */
export function loadSecretaryStatus(): void {
  api
    .hubSecretaryStatus()
    .then((list) => hubStore.secretaryStatusLoaded(list))
    .catch((e) => report("caricamento dello stato dei segretari", e));
}

/** `hub-secretary-status`: l'elenco completo dei segretari svegli. */
export function onSecretaryStatus(list: HubSecretaryStatus[]): void {
  hubStore.secretaryStatusLoaded(list);
}

/** Accende o spegne l'HUB. Spegnerlo riporta ai terminali. Rifiuta con l'errore del backend. */
export async function setHubEnabled(enabled: boolean): Promise<void> {
  const settings = await api.hubSetEnabled(enabled);
  hubStore.settingsLoaded(settings);
  if (!settings.enabled) uiStore.setWorld("terminals");
}

/** Legge i riassunti dei thread di un canale. */
export function loadChannelThreads(channel: string): void {
  api
    .hubListThreads(channel)
    .then((list) => {
      hubStore.threadsLoaded(channel, list);
      hubStore.seenPruned(hubChannelIdsOf(workspacesStore.getState()));
    })
    .catch((e) => report("caricamento dei thread dell'Agents HUB", e, { notify: true }));
}

/** L'elenco di ogni canale, per i segnalini di chi aspetta l'utente (a HUB acceso, in entrambi i mondi). */
export function loadAllChannels(channels: readonly string[]): void {
  for (const channel of channels) loadChannelThreads(channel);
}

/** Rilegge la conversazione del thread aperto. */
export function loadThread(channel: string, threadId: string): void {
  api
    .hubGetThread(channel, threadId)
    .then((thread) => {
      hubStore.threadLoaded(thread);
      void markOpenChatSeen();
    })
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

/**
 * Invia un messaggio dell'utente, con l'eventuale messaggio a cui risponde e gli
 * id degli allegati già nello staging. Rifiuta con l'errore del backend.
 */
export async function sendMessage(
  channel: string,
  threadId: string,
  body: string,
  replyTo: string | null,
  attachments: string[] = [],
): Promise<void> {
  const message = await api.hubSend(channel, threadId, body, replyTo, attachments);
  hubStore.messageAdded(threadId, message);
}

/** `hub-update`: aggiorna la riga del thread e rilegge la conversazione se è quella aperta e non è aggiornata. */
export function onHubUpdate(summary: HubThreadSummary): void {
  const state = hubStore.getState();
  if (!state.enabled) return;
  hubStore.summaryUpdated(summary);
  if (threadNeedsReload(state.thread, summary)) loadThread(summary.channel, summary.id);
  void markOpenChatSeen();
}

/**
 * Chiude (`closed`) o riapre il thread e ne aggiorna subito riga e conversazione,
 * senza aspettare `hub-update`. Rifiuta con l'errore del backend.
 */
export async function setThreadClosed(channel: string, threadId: string, closed: boolean): Promise<void> {
  onHubUpdate(await api.hubSetThreadState(channel, threadId, closed));
}

/**
 * Risponde a una domanda di un worker con `text` (un'opzione suggerita) e rilegge
 * subito il thread: la consegna è sincrona, quindi l'esito ("Risposta inviata" o
 * l'errore con il suggerimento del terminale) è già scritto e la scheda lo mostra
 * senza aspettare `hub-update`. Rifiuta con l'errore del backend se l'invio fallisce.
 */
export async function answerQuestion(
  channel: string,
  threadId: string,
  questionMessageId: string,
  text: string,
): Promise<void> {
  await sendMessage(channel, threadId, text, questionMessageId);
  try {
    hubStore.threadLoaded(await api.hubGetThread(channel, threadId));
  } catch (e) {
    report("aggiornamento della conversazione dopo la risposta", e);
  }
}

/**
 * Decide la richiesta di azione di una scheda (`confirm`: conferma, altrimenti annulla)
 * e rilegge il thread: la scheda e la riga dell'esito sono già scritte e si mostrano
 * senza aspettare `hub-update`. Rifiuta con l'errore del backend (anche «già decisa»).
 */
export async function resolveActionRequest(
  channel: string,
  threadId: string,
  messageId: string,
  confirm: boolean,
): Promise<void> {
  await api.hubResolveActionRequest(channel, threadId, messageId, confirm);
  try {
    hubStore.threadLoaded(await api.hubGetThread(channel, threadId));
  } catch (e) {
    report("aggiornamento della conversazione dopo la decisione", e);
  }
}

/** Apre l'Agents HUB su un thread (click su una notifica di sistema). Con l'HUB spento non fa nulla. */
export function openHubThread(channel: string, threadId: string): void {
  if (!hubStore.getState().enabled) return;
  uiStore.setWorld("hub");
  selectChannel(channel);
  selectThread(channel, threadId);
}

/**
 * Apre l'Agents HUB sul thread con la domanda di un task. Il backend non dice a
 * quale thread appartiene un task: si leggono, fino a `THREAD_SEARCH_LIMIT`, i
 * thread del canale (prima quelli che aspettano l'utente). Se non lo trova,
 * resta sull'elenco del canale.
 */
export async function openTaskThread(channel: string, taskId: string): Promise<void> {
  uiStore.setWorld("hub");
  selectChannel(channel);
  try {
    const list = await api.hubListThreads(channel);
    hubStore.threadsLoaded(channel, list);
    for (const id of threadsToSearch(list)) {
      const thread = await api.hubGetThread(channel, id);
      if (hubStore.getState().channel !== channel) return;
      if (threadAsksTask(thread, channel, taskId)) {
        hubStore.threadSelected(id);
        hubStore.threadLoaded(thread);
        void markOpenChatSeen();
        return;
      }
    }
  } catch (e) {
    report("ricerca del thread della domanda", e, { notify: true });
  }
}
