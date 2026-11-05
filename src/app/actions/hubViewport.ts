// Cosa guarda l'utente adesso nell'Agents HUB: finestra, mondo, canale e thread.

import type { HubViewport } from "../../lib/hubNotify";
import { appHasFocus } from "../../lib/systemNotify";
import { hubStore } from "../../stores/hub";
import { uiStore } from "../../stores/ui";
import { hubShown } from "../../stores/view";

export async function currentViewport(): Promise<HubViewport> {
  const hub = hubStore.getState();
  return {
    focused: await appHasFocus(),
    hubShown: hubShown(uiStore.getState(), hub),
    channel: hub.channel,
    threadId: hub.threadId,
  };
}
