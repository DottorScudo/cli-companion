import { onHubUpdate } from "../app/actions/hub";
import { useTauriEvent } from "./useTauriEvent";

/** Aggiornamenti dell'Agents HUB dal backend (`hub-update`): con l'HUB spento non arrivano o si ignorano. */
export function useHubSync(): void {
  useTauriEvent("hub-update", onHubUpdate);
}
