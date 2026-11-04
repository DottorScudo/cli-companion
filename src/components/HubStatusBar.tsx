import { MessagesSquare } from "lucide-react";
import { useHubWaitingTotal } from "../hooks/useHubChannels";
import { useStore } from "../hooks/useStore";
import { terminalCountText } from "../lib/agents";
import { waitingText } from "../lib/hub";
import { tabsStore } from "../stores/tabs";
import { StatusBarFrame } from "./StatusBar";

/** Barra di stato del mondo Agents HUB: thread in attesa a sinistra, terminali aperti a destra. */
export default function HubStatusBar() {
  const waiting = useHubWaitingTotal();
  const terminalCount = useStore(tabsStore, (s) => s.length);

  return (
    <StatusBarFrame>
      <span className="flex items-center gap-1.5">
        <MessagesSquare className="!size-3" aria-hidden="true" />
        Agents HUB
      </span>
      {waiting > 0 && <span className="text-status-attention">{waitingText(waiting)}</span>}
      <span className="flex-1" />
      <span>{terminalCountText(terminalCount)}</span>
    </StatusBarFrame>
  );
}
