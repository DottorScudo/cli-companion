import { useEffect } from "react";
import { loadThread, selectChannel } from "../../app/actions/hub";
import { useHubChannels } from "../../hooks/useHubChannels";
import { useStore } from "../../hooks/useStore";
import { GENERAL_CHANNEL, channelName } from "../../lib/hub";
import { hubStore } from "../../stores/hub";
import HubConversation from "./HubConversation";
import HubThreadList from "./HubThreadList";

/**
 * Centro del mondo Agents HUB: thread del canale e conversazione (la colonna
 * dei canali, o delle chat nella vista messenger, è `HubSidebar`, a sinistra).
 * Nella vista messenger i thread stanno nella sidebar e la conversazione ha
 * tutta la larghezza. All'apertura rilegge il thread rimasto aperto; poi segue
 * `hub-update`, senza interrogazioni periodiche.
 */
export default function HubView() {
  const { channels, current } = useHubChannels();
  const channel = useStore(hubStore, (s) => s.channel);
  const messenger = useStore(hubStore, (s) => s.viewMode === "messenger");

  useEffect(() => {
    const { channel: openChannel, threadId } = hubStore.getState();
    if (threadId !== null) loadThread(openChannel, threadId);
  }, []);

  // Il workspace del canale selezionato non c'è più (rimosso o archiviato).
  useEffect(() => {
    if (current !== channel) selectChannel(GENERAL_CHANNEL);
  }, [current, channel]);

  return (
    <div className="flex h-full min-h-0">
      {!messenger && <HubThreadList channel={current} channelName={channelName(channels, current)} />}
      <HubConversation channel={current} channelName={channelName(channels, current)} messenger={messenger} />
    </div>
  );
}
