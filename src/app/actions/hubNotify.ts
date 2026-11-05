// Agents HUB: notifiche di sistema dei thread (docs/HUB.MD › Notifiche). Il
// backend decide cosa notificare e manda `hub-notify`; qui si decide se
// mostrarla (finestra, mondo e thread aperto) e si riproduce il suono, una
// volta sola per notifica mostrata.

import type { HubNotice, HubThreadRef } from "../../lib/tauri";
import { shouldShowHubNotice, soundOf } from "../../lib/hubNotify";
import { playAttention, playDing } from "../../lib/notifySound";
import { notifyHubThread } from "../../lib/systemNotify";
import { hubStore } from "../../stores/hub";
import { openHubThread } from "./hub";
import { currentViewport } from "./hubViewport";

/**
 * `hub-notify`: notifica di sistema e suono, tranne se l'utente sta guardando
 * quel thread. Il backend ha gia' controllato l'interruttore; con l'HUB spento
 * nel frattempo non si mostra nulla.
 */
export async function onHubNotify(notice: HubNotice): Promise<void> {
  if (!hubStore.getState().enabled) return;
  if (!shouldShowHubNotice(await currentViewport(), notice)) return;
  if (soundOf(notice.sound) === "attention") playAttention();
  else playDing();
  await notifyHubThread(notice.channel, notice.thread_id, notice.title, notice.body);
}

/** `hub-notification-activated`: la finestra e' gia' in primo piano (backend); si apre il thread della notifica. */
export function onHubNotificationActivated(target: HubThreadRef): void {
  openHubThread(target.channel, target.thread_id);
}
