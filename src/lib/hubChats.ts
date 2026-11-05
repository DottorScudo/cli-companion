// Agents HUB, vista messenger: elenco delle chat (un thread = una chat, di tutti
// i canali visibili) e anteprima dell'ultimo messaggio, calcolata solo nel client.
// Funzioni pure a zero dipendenze, importabili sia dall'app sia dai test `node --test`.

import type { HubMessage } from "../bindings/HubMessage.ts";
import type { HubThread } from "../bindings/HubThread.ts";
import type { HubThreadSummary } from "../bindings/HubThreadSummary.ts";
import { authorLabel, excerpt, filterThreads } from "./hub.ts";
import type { HubChannel, ThreadFilter, ThreadsByChannel } from "./hub.ts";
import { eventLook } from "./hubEvents.ts";

/** Chat di cui si legge l'anteprima: le più recenti dell'elenco mostrato. */
export const CHAT_PREVIEW_LIMIT = 30;

/** Letture di conversazioni in parallelo per le anteprime. */
export const CHAT_PREVIEW_CONCURRENCY = 4;

/** Caratteri dell'anteprima su una riga. */
const PREVIEW_MAX_CHARS = 80;

export interface ChatEntry {
  thread: HubThreadSummary;
  /** "Generale" o il nome del workspace: l'etichetta della card. */
  channelName: string;
}

/** Ultimo messaggio di un thread, come lo mostra la card della chat. */
export interface ThreadPreview {
  /** `updated_at` del thread quando l'anteprima è stata calcolata. */
  updatedAt: number;
  /** "Tu", "@segretario" o "Sistema"; vuoto se il thread non ha messaggi. */
  author: string;
  /** Una riga; vuoto se il thread non ha messaggi (nessuna anteprima). */
  text: string;
  /** Momento dell'ultimo messaggio; 0 se il thread non ha messaggi. */
  at: number;
}

const byRecency = (a: ChatEntry, b: ChatEntry): number =>
  b.thread.updated_at - a.thread.updated_at || (a.thread.id < b.thread.id ? -1 : a.thread.id > b.thread.id ? 1 : 0);

/**
 * Le chat dei canali dati (i visibili), dalla più recente (a parità, per id).
 * `filter` è quello della vista per canale; `query` cerca, senza badare alle
 * maiuscole né agli spazi ai bordi, nel titolo e nel nome del workspace;
 * `onlyChannel` limita a un canale (`null` = tutti).
 */
export function chatList(
  threadsByChannel: ThreadsByChannel,
  channels: readonly HubChannel[],
  filter: ThreadFilter,
  query: string,
  onlyChannel: string | null = null,
): ChatEntry[] {
  const needle = query.trim().toLowerCase();
  const entries: ChatEntry[] = [];
  for (const channel of channels) {
    if (onlyChannel !== null && channel.id !== onlyChannel) continue;
    const name = channel.name;
    for (const thread of filterThreads(threadsByChannel[channel.id] ?? [], filter)) {
      if (needle === "" || thread.title.toLowerCase().includes(needle) || name.toLowerCase().includes(needle)) {
        entries.push({ thread, channelName: name });
      }
    }
  }
  return entries.sort(byRecency);
}

/** Testo su una riga di un messaggio: l'evento col suo testo breve, un messaggio con soli allegati con il loro riassunto. */
export function messagePreviewText(message: HubMessage): string {
  const body = excerpt(message.body, PREVIEW_MAX_CHARS);
  if (body !== "") return body;
  if (message.attachments.length === 1) return `📎 ${message.attachments[0].name}`;
  if (message.attachments.length > 1) return `📎 ${message.attachments.length} allegati`;
  return message.event ? eventLook(message.event.kind).label : "";
}

/** "Tu", "@segretario" o "Sistema". */
export function previewAuthor(author: HubMessage["author"]): string {
  return author.type === "bot" ? `@${authorLabel(author)}` : authorLabel(author);
}

/** Anteprima di un thread letto: il suo ultimo messaggio; vuota se non ne ha. */
export function previewOf(thread: HubThread): ThreadPreview {
  const last = thread.messages[thread.messages.length - 1];
  if (last === undefined) return { updatedAt: thread.updated_at, author: "", text: "", at: 0 };
  return {
    updatedAt: thread.updated_at,
    author: previewAuthor(last.author),
    text: messagePreviewText(last),
    at: last.created_at,
  };
}

/**
 * Chat da rileggere per l'anteprima: tra le prime `limit` dell'elenco, quelle
 * senza anteprima o con un'anteprima più vecchia del thread (`updated_at` cambiato).
 */
export function previewsToRead(
  threads: readonly HubThreadSummary[],
  previews: Readonly<Record<string, ThreadPreview>>,
  limit = CHAT_PREVIEW_LIMIT,
): HubThreadSummary[] {
  return threads.slice(0, limit).filter((t) => (previews[t.id]?.updatedAt ?? -Infinity) < t.updated_at);
}

const samePreview = (a: ThreadPreview, b: ThreadPreview): boolean =>
  a.updatedAt === b.updatedAt && a.author === b.author && a.text === b.text && a.at === b.at;

/** Aggiunge o sostituisce l'anteprima di un thread; una più vecchia di quella nota non la sostituisce. */
export function withPreview(
  previews: Readonly<Record<string, ThreadPreview>>,
  threadId: string,
  preview: ThreadPreview,
): Readonly<Record<string, ThreadPreview>> {
  const known = previews[threadId];
  if (known && (known.updatedAt > preview.updatedAt || samePreview(known, preview))) return previews;
  return { ...previews, [threadId]: preview };
}
