import { memo } from "react";
import { useStore } from "../../hooks/useStore";
import { hubStore } from "../../stores/hub";
import HubChannelList from "./HubChannelList";
import HubChatList from "./HubChatList";
import HubSecretaryList from "./HubSecretaryList";
import HubViewSwitch from "./HubViewSwitch";

/**
 * Colonna di sinistra del mondo Agents HUB (260px, come la sidebar dei
 * workspace): l'interruttore Canali | Chat, poi i canali (Generale e un canale
 * per workspace) o le chat di tutti i canali, e sotto i segretari con il loro stato.
 */
function HubSidebar() {
  const messenger = useStore(hubStore, (s) => s.viewMode === "messenger");
  return (
    <nav
      aria-label={messenger ? "Chat" : "Canali"}
      className="flex w-[260px] flex-shrink-0 flex-col overflow-hidden border-r bg-base"
    >
      <HubViewSwitch />
      {messenger ? <HubChatList /> : <HubChannelList />}
      <HubSecretaryList />
    </nav>
  );
}

export default memo(HubSidebar);
