import { useEffect } from "react";
import { loadAllChannels, onHubUpdate } from "../app/actions/hub";
import { hubStore } from "../stores/hub";
import { useHubChannelIds } from "./useHubChannels";
import { useStore } from "./useStore";
import { useTauriEvent } from "./useTauriEvent";

/**
 * Agents HUB: `hub-update` dal backend (con l'HUB spento non arriva o si ignora)
 * e, a HUB acceso, l'elenco di ogni canale, per i segnalini di chi aspetta
 * l'utente anche nel mondo dei terminali; poi `hub-update` li tiene aggiornati.
 */
export function useHubSync(): void {
  useTauriEvent("hub-update", onHubUpdate);

  const enabled = useStore(hubStore, (s) => s.enabled);
  const channelIds = useHubChannelIds();
  useEffect(() => {
    if (enabled) loadAllChannels(channelIds);
  }, [enabled, channelIds]);
}
