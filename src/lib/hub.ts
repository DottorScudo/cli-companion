// Agents HUB (docs/HUB.MD): canali, elenco dei thread, testo dei messaggi e
// decisioni di aggiornamento. Funzioni pure a zero dipendenze, importabili sia
// dall'app sia dai test `node --test`.

import type { HubMessage } from "../bindings/HubMessage.ts";
import type { HubThread } from "../bindings/HubThread.ts";
import type { HubThreadSummary } from "../bindings/HubThreadSummary.ts";

/** Canale riservato ai thread che non appartengono a un workspace. */
export const GENERAL_CHANNEL = "general";

/** Lunghezza massima del testo di un messaggio (come il backend, `hub/text.rs`). */
export const MESSAGE_MAX_CHARS = 8000;

/** Lunghezza massima del titolo di un thread (come il backend). */
export const THREAD_TITLE_MAX_CHARS = 120;

export interface HubChannel {
  /** "general" oppure l'id del workspace. */
  id: string;
  name: string;
}

export type ThreadFilter = "open" | "all";

/** Generale in cima, poi un canale per workspace nell'ordine ricevuto (quello della sidebar). */
export function hubChannels(workspaces: readonly { id: string; name: string }[]): HubChannel[] {
  return [{ id: GENERAL_CHANNEL, name: "Generale" }, ...workspaces.map((w) => ({ id: w.id, name: w.name }))];
}

/** Il canale scelto, o il Generale se non esiste più (workspace rimosso o archiviato). */
export function resolveChannel(channel: string, channels: readonly HubChannel[]): string {
  return channels.some((c) => c.id === channel) ? channel : GENERAL_CHANNEL;
}

/** Thread da mostrare nell'elenco; l'ordine (dal più recente) è quello del backend. */
export function filterThreads(threads: readonly HubThreadSummary[], filter: ThreadFilter): HubThreadSummary[] {
  return filter === "all" ? [...threads] : threads.filter((t) => t.state === "open");
}

/** Dal più recente; a parità di aggiornamento, il più recente per creazione. */
export function sortThreads(threads: readonly HubThreadSummary[]): HubThreadSummary[] {
  return [...threads].sort((a, b) => b.updated_at - a.updated_at || b.created_at - a.created_at);
}

/** Inserisce o sostituisce il riassunto di un thread; una versione più vecchia di quella nota non la sostituisce. */
export function upsertSummary(list: readonly HubThreadSummary[], summary: HubThreadSummary): HubThreadSummary[] {
  const known = list.find((t) => t.id === summary.id);
  if (known && known.updated_at > summary.updated_at) return list as HubThreadSummary[];
  return sortThreads([...list.filter((t) => t.id !== summary.id), summary]);
}

/** Almeno un thread dell'elenco aspetta l'utente (badge del canale). */
export function channelWaiting(threads: readonly HubThreadSummary[] | undefined): boolean {
  return threads?.some((t) => t.waiting_user) ?? false;
}

/** Riassunto di un thread appena letto (la riga dell'elenco); `waiting_user` lo calcola il backend, un thread nuovo non aspetta. */
export function summarizeThread(thread: HubThread): HubThreadSummary {
  return {
    id: thread.id,
    channel: thread.channel,
    title: thread.title,
    state: thread.state,
    task_count: thread.task_ids.length,
    message_count: thread.messages.length,
    waiting_user: false,
    created_at: thread.created_at,
    updated_at: thread.updated_at,
  };
}

/** La conversazione aperta va riletta? (nel riassunto c'è più di quanto abbiamo) */
export function threadNeedsReload(thread: HubThread | null, summary: HubThreadSummary): boolean {
  if (thread === null || thread.id !== summary.id) return false;
  return summary.message_count !== thread.messages.length || summary.updated_at > thread.updated_at;
}

/** Il messaggio citato da una risposta; assente se è stato potato dai limiti del backend. */
export function replyTarget(thread: HubThread, replyTo: string | null): HubMessage | null {
  if (replyTo === null) return null;
  return thread.messages.find((m) => m.id === replyTo) ?? null;
}

/** Aggiunge un messaggio al thread aperto (senza duplicati: può arrivare anche da `hub-update`). */
export function appendMessage(thread: HubThread, message: HubMessage): HubThread {
  if (thread.messages.some((m) => m.id === message.id)) return thread;
  return {
    ...thread,
    messages: [...thread.messages, message],
    updated_at: Math.max(thread.updated_at, message.created_at),
  };
}

export type BodyCheck = { ok: true; body: string } | { ok: false; reason: string };

/** Testo da inviare: senza spazi ai bordi, non vuoto e nel limite del backend. */
export function checkBody(raw: string): BodyCheck {
  const body = raw.trim();
  if (body === "") return { ok: false, reason: "Scrivi un messaggio." };
  if ([...body].length > MESSAGE_MAX_CHARS) {
    return { ok: false, reason: `Il messaggio supera i ${MESSAGE_MAX_CHARS} caratteri.` };
  }
  return { ok: true, body };
}

/** Titolo da inviare: una riga, non vuota e nel limite del backend. */
export function checkTitle(raw: string): BodyCheck {
  const body = raw.trim();
  if (body === "") return { ok: false, reason: "Scrivi un titolo." };
  if ([...body].length > THREAD_TITLE_MAX_CHARS) {
    return { ok: false, reason: `Il titolo supera i ${THREAD_TITLE_MAX_CHARS} caratteri.` };
  }
  return { ok: true, body };
}

/** Anteprima su una riga del testo di un messaggio (per la citazione di una risposta). */
export function excerpt(text: string, max = 120): string {
  const flat = text.replace(/\s+/g, " ").trim();
  const chars = [...flat];
  return chars.length > max ? `${chars.slice(0, max - 1).join("")}…` : flat;
}

/** Nome mostrato per l'autore di un messaggio. */
export function authorLabel(author: HubMessage["author"]): string {
  switch (author.type) {
    case "user":
      return "Tu";
    case "system":
      return "Sistema";
    case "bot":
      return author.name;
  }
}
