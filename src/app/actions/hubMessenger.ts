// Agents HUB, vista messenger: modalità di vista e apertura delle chat. Una chat è
// un thread: aprirla imposta canale e thread, quindi notifiche e non letti usano
// le stesse regole della vista per canale.

import type { HubViewMode } from "../../lib/hubViewMode";
import { hubStore } from "../../stores/hub";
import { createThread, selectThread } from "./hub";
import { markOpenChatSeen } from "./hubChatState";

/** Passa alla vista per canale o a quella messenger (preferenza globale, ricordata). */
export function setHubViewMode(mode: HubViewMode): void {
  hubStore.viewModeSet(mode);
  void markOpenChatSeen();
}

/** Apre una chat: il canale (il thread aperto resta se non cambia) e il thread, che si rilegge. */
export function openChat(channel: string, threadId: string): void {
  hubStore.channelSelected(channel);
  selectThread(channel, threadId);
  void markOpenChatSeen();
}

/** Crea una chat nel canale e la apre. Rifiuta con l'errore del backend. */
export async function createChat(channel: string, title: string): Promise<void> {
  await createThread(channel, title);
  await markOpenChatSeen();
}
