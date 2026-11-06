import { useEffect } from "react";
import {
  loadAllChannels,
  loadSecretaryStatus,
  onHubUpdate,
  onSecretaryStatus,
  onThreadRemoved,
} from "../app/actions/hub";
import { onHubNotificationActivated, onHubNotify } from "../app/actions/hubNotify";
import { markOpenChatSeen } from "../app/actions/hubChatState";
import { hubStore } from "../stores/hub";
import { useHubChannelIds } from "./useHubChannels";
import { useStore } from "./useStore";
import { useTauriEvent } from "./useTauriEvent";

/**
 * Agents HUB: `hub-update`, `hub-thread-removed` (thread eliminato), `hub-secretary-status` (chi è sveglio) e
 * `hub-notify` (notifiche di sistema, con il click `hub-notification-activated`)
 * dal backend (con l'HUB spento non arrivano o si ignorano) e, a HUB acceso, l'elenco di
 * ogni canale, per i segnalini di chi aspetta l'utente anche nel mondo dei
 * terminali; poi `hub-update` li tiene aggiornati. Al ritorno del focus sulla
 * finestra segna letta la chat aperta (vista messenger).
 */
export function useHubSync(): void {
  useTauriEvent("hub-update", onHubUpdate);
  useTauriEvent("hub-thread-removed", onThreadRemoved);
  useTauriEvent("hub-secretary-status", onSecretaryStatus);
  useTauriEvent("hub-notify", (notice) => void onHubNotify(notice));
  useTauriEvent("hub-notification-activated", onHubNotificationActivated);

  useEffect(() => {
    const onFocus = (): void => void markOpenChatSeen();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, []);

  const enabled = useStore(hubStore, (s) => s.enabled);
  const channelIds = useHubChannelIds();
  useEffect(() => {
    if (enabled) loadAllChannels(channelIds);
  }, [enabled, channelIds]);
  useEffect(() => {
    if (enabled) loadSecretaryStatus();
  }, [enabled]);
}
