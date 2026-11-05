// Agents HUB, vista messenger: chat non lette, solo lato client. Per ogni thread
// si ricorda quanti messaggi l'utente ha visto (`message_count` al momento in cui
// il thread era aperto e in vista); una chat è non letta se ne ha di più. Funzioni
// pure a zero dipendenze, importabili sia dall'app sia dai test `node --test`.

import type { HubThread } from "../bindings/HubThread.ts";
import type { HubThreadSummary } from "../bindings/HubThreadSummary.ts";
import type { StorageLike } from "./taskPanelPrefs.ts";

/** Messaggi visti per id del thread. */
export type SeenMap = Readonly<Record<string, number>>;

/** Riassunti dei thread per canale, come li tiene `stores/hub.ts`. */
type SummariesByChannel = Readonly<Record<string, readonly HubThreadSummary[] | undefined>>;

/** Chiave di localStorage: oggetto JSON `{ [threadId]: messaggi visti }`. */
export const HUB_SEEN_STORAGE_KEY = "cli-companion.hub.seen";

const isCount = (v: unknown): v is number => typeof v === "number" && Number.isSafeInteger(v) && v >= 0;

/** Legge la mappa; `null` se non c'è nulla di salvato (primo avvio) o non si legge. */
export function readSeen(storage: StorageLike | null | undefined): SeenMap | null {
  try {
    const raw = storage?.getItem(HUB_SEEN_STORAGE_KEY);
    if (!raw) return null;
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return null;
    const out: Record<string, number> = {};
    for (const [id, count] of Object.entries(parsed)) if (isCount(count)) out[id] = count;
    return out;
  } catch {
    return null;
  }
}

/** Scrive la mappa; gli errori di storage sono ignorati (i non letti si ricalcolano al riavvio). */
export function writeSeen(storage: StorageLike | null | undefined, seen: SeenMap): void {
  try {
    storage?.setItem(HUB_SEEN_STORAGE_KEY, JSON.stringify(seen));
  } catch {
    // Storage non disponibile (finestra privata, quota): al riavvio i non letti si ricalcolano.
  }
}

/** La chat ha messaggi che l'utente non ha ancora visto. Un thread mai visto ha tutti i messaggi da vedere. */
export function isUnread(seen: SeenMap, summary: Pick<HubThreadSummary, "id" | "message_count">): boolean {
  return summary.message_count > (seen[summary.id] ?? 0);
}

/** Segna visti `count` messaggi del thread; non torna mai indietro e, se non cambia nulla, ritorna la stessa mappa. */
export function markSeen(seen: SeenMap, threadId: string, count: number): SeenMap {
  if (!isCount(count) || (threadId in seen && count <= seen[threadId])) return seen;
  if (!(threadId in seen) && count === 0) return seen;
  return { ...seen, [threadId]: count };
}

/** Primo avvio: i thread già esistenti e senza voce si considerano letti. Stessa mappa se non c'è nulla da aggiungere. */
export function baselineSeen(seen: SeenMap, summaries: readonly HubThreadSummary[]): SeenMap {
  const missing = summaries.filter((t) => !(t.id in seen));
  if (missing.length === 0) return seen;
  return { ...seen, ...Object.fromEntries(missing.map((t) => [t.id, t.message_count])) };
}

/**
 * Toglie gli id che non esistono più tra i thread dei canali dati. Finché uno di
 * quei canali non è stato letto non si pota: i suoi thread sembrerebbero spariti.
 */
export function pruneSeen(seen: SeenMap, threads: SummariesByChannel, channelIds: readonly string[]): SeenMap {
  const lists = channelIds.map((id) => threads[id]);
  if (lists.some((list) => list === undefined)) return seen;
  const alive = new Set(lists.flatMap((list) => (list ?? []).map((t) => t.id)));
  const ids = Object.keys(seen);
  const kept = ids.filter((id) => alive.has(id));
  return kept.length === ids.length ? seen : Object.fromEntries(kept.map((id) => [id, seen[id]]));
}

/**
 * Messaggi del thread aperto da segnare visti: quelli del riassunto (aggiornato
 * da `hub-update`) o, se manca, quelli della conversazione letta. `null` se non
 * c'è nessun thread aperto.
 */
export function openThreadCount(
  threads: SummariesByChannel,
  channel: string,
  threadId: string | null,
  thread: HubThread | null,
): number | null {
  if (threadId === null) return null;
  const summary = threads[channel]?.find((t) => t.id === threadId);
  if (summary) return Math.max(summary.message_count, thread?.id === threadId ? thread.messages.length : 0);
  return thread?.id === threadId ? thread.messages.length : null;
}
