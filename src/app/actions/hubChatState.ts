// Agents HUB, vista messenger: anteprime delle chat e chat lette. Solo client:
// le anteprime si leggono con `hub_get_thread` (nessun dato in più dal backend) e i
// messaggi visti stanno nello store, salvati in localStorage. Con la vista per
// canale non si legge e non si segna nulla.

import { api } from "../../lib/tauri";
import type { HubThreadSummary } from "../../lib/tauri";
import { report } from "../../lib/errors";
import { CHAT_PREVIEW_CONCURRENCY, previewOf, previewsToRead } from "../../lib/hubChats";
import { threadIsInView } from "../../lib/hubNotify";
import { mapLimit } from "../../lib/mapLimit";
import { openThreadCount } from "../../lib/hubUnread";
import { hubStore } from "../../stores/hub";
import { uiStore } from "../../stores/ui";
import { hubShown } from "../../stores/view";
import { currentViewport } from "./hubViewport";

/** Letture di anteprime in corso (`id:updated_at`): la stessa non parte due volte. */
const inFlight = new Set<string>();

function messengerShown(): boolean {
  const hub = hubStore.getState();
  return hub.enabled && hub.viewMode === "messenger" && hubShown(uiStore.getState(), hub);
}

async function readPreview(thread: HubThreadSummary): Promise<void> {
  const key = `${thread.id}:${thread.updated_at}`;
  if (inFlight.has(key)) return;
  inFlight.add(key);
  try {
    const read = await api.hubGetThread(thread.channel, thread.id);
    hubStore.previewLoaded(thread.id, previewOf(read));
  } catch (e) {
    report("lettura dell'anteprima della chat", e);
  } finally {
    inFlight.delete(key);
  }
}

/**
 * Aggiorna le anteprime delle chat mostrate (già ordinate dalla più recente): le
 * prime `CHAT_PREVIEW_LIMIT` senza anteprima o con `updated_at` cambiato, al più
 * `CHAT_PREVIEW_CONCURRENCY` alla volta. Un errore non blocca l'elenco. Non fa
 * nulla senza la vista messenger o con l'HUB non mostrato.
 */
export async function loadChatPreviews(threads: readonly HubThreadSummary[]): Promise<void> {
  if (!messengerShown()) return;
  const stale = previewsToRead(threads, hubStore.getState().previews);
  await mapLimit(stale, CHAT_PREVIEW_CONCURRENCY, readPreview);
}

/**
 * Segna letta la chat aperta, ma solo se è in vista (finestra in primo piano,
 * mondo HUB, quel canale e quel thread: come per le notifiche). Da chiamare
 * all'apertura e a ogni messaggio ricevuto.
 */
export async function markOpenChatSeen(): Promise<void> {
  const before = hubStore.getState();
  if (!before.enabled || before.viewMode !== "messenger" || before.threadId === null) return;
  const viewport = await currentViewport();
  if (!threadIsInView(viewport, { channel: before.channel, thread_id: before.threadId })) return;
  const now = hubStore.getState();
  const count = openThreadCount(now.threads, now.channel, now.threadId, now.thread);
  if (now.threadId !== null && count !== null) hubStore.threadSeen(now.threadId, count);
}
