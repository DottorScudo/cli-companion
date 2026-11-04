import { memo } from "react";
import { Hash } from "lucide-react";
import { selectChannel } from "../../app/actions/hub";
import { useHubChannels } from "../../hooks/useHubChannels";
import { useStore } from "../../hooks/useStore";
import { waitingCount, waitingText } from "../../lib/hub";
import type { HubChannel } from "../../lib/hub";
import { cn } from "../../lib/utils";
import { hubStore } from "../../stores/hub";
import WaitingBadge from "./WaitingBadge";

const ChannelItem = memo(function ChannelItem({ channel, selected }: { channel: HubChannel; selected: boolean }) {
  const waiting = useStore(hubStore, (s) => waitingCount(s.threads[channel.id]));
  return (
    <li>
      <button
        type="button"
        onClick={() => selectChannel(channel.id)}
        aria-current={selected ? "true" : undefined}
        aria-label={waiting > 0 ? `${channel.name}, ${waitingText(waiting)}` : channel.name}
        className={cn(
          "flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-[13px] transition-colors",
          selected ? "bg-surface font-medium text-foreground" : "text-text-secondary hover:bg-surface/60",
        )}
      >
        <Hash className="!size-3.5 flex-shrink-0" aria-hidden="true" />
        <span className="min-w-0 flex-1 truncate">{channel.name}</span>
        {waiting > 0 && <WaitingBadge count={waiting} />}
      </button>
    </li>
  );
});

/**
 * Colonna di sinistra del mondo Agents HUB (260px, come la sidebar dei
 * workspace): Generale in cima, poi un canale per workspace visibile.
 */
function HubSidebar() {
  const { channels, current } = useHubChannels();
  return (
    <nav aria-label="Canali" className="flex w-[260px] flex-shrink-0 flex-col overflow-hidden border-r bg-base">
      <h1 className="pb-2 pl-4 pr-3 pt-3.5 text-[11px] font-semibold uppercase tracking-[0.08em] text-faint">Canali</h1>
      <ul className="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto px-2">
        {channels.map((c) => (
          <ChannelItem key={c.id} channel={c} selected={c.id === current} />
        ))}
      </ul>
    </nav>
  );
}

export default memo(HubSidebar);
