import { memo } from "react";
import { Hash } from "lucide-react";
import { selectChannel } from "../../app/actions/hub";
import { useStore } from "../../hooks/useStore";
import { channelWaiting } from "../../lib/hub";
import type { HubChannel } from "../../lib/hub";
import { cn } from "../../lib/utils";
import { hubStore } from "../../stores/hub";

const ChannelItem = memo(function ChannelItem({ channel, selected }: { channel: HubChannel; selected: boolean }) {
  const waiting = useStore(hubStore, (s) => channelWaiting(s.threads[channel.id]));
  return (
    <li>
      <button
        type="button"
        onClick={() => selectChannel(channel.id)}
        aria-current={selected ? "true" : undefined}
        className={cn(
          "flex w-full items-center gap-2 rounded-md px-2 py-1.5 text-left text-[13px] transition-colors",
          selected ? "bg-accent font-medium text-foreground" : "text-muted-foreground hover:bg-accent/50",
        )}
      >
        <Hash className="!size-3.5 flex-shrink-0" aria-hidden="true" />
        <span className="min-w-0 flex-1 truncate">{channel.name}</span>
        {waiting && (
          <span
            className="h-2 w-2 flex-shrink-0 rounded-full bg-status-attention"
            role="img"
            aria-label="Un thread aspetta una tua risposta"
            title="Un thread aspetta una tua risposta"
          />
        )}
      </button>
    </li>
  );
});

interface Props {
  channels: readonly HubChannel[];
  selected: string;
}

/** Colonna dei canali: Generale in cima, poi un canale per workspace. */
export default function HubChannelList({ channels, selected }: Props) {
  return (
    <nav aria-label="Canali" className="w-[200px] flex-shrink-0 overflow-y-auto border-r bg-base p-2">
      <p className="px-2 pb-1 text-[10px] font-semibold uppercase tracking-[0.08em] text-faint">Canali</p>
      <ul className="flex flex-col gap-0.5">
        {channels.map((c) => (
          <ChannelItem key={c.id} channel={c} selected={c.id === selected} />
        ))}
      </ul>
    </nav>
  );
}
