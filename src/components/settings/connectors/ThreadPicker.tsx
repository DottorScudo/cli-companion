import { useEffect } from "react";
import { loadChannelThreads } from "../../../app/actions/hub";
import { useStore } from "../../../hooks/useStore";
import { useHubChannels } from "../../../hooks/useHubChannels";
import { hubStore } from "../../../stores/hub";
import ChannelSelect from "../../hub/ChannelSelect";
import { selectClass } from "../shared";

interface Props {
  channel: string;
  threadId: string;
  onChange: (channel: string, threadId: string) => void;
  /** Il canale è deciso altrove (il thread da cui si parte): si sceglie solo il thread. */
  channelLocked?: boolean;
  disabled?: boolean;
  label: string;
}

/** Sceglie un thread dell'HUB: prima il canale (Generale o un workspace), poi il thread. */
export default function ThreadPicker({ channel, threadId, onChange, channelLocked = false, disabled, label }: Props) {
  const { channels } = useHubChannels();
  const hubEnabled = useStore(hubStore, (s) => s.enabled);
  const threads = useStore(hubStore, (s) => s.threads[channel]);

  useEffect(() => {
    if (hubEnabled && threads === undefined) loadChannelThreads(channel);
  }, [hubEnabled, channel, threads]);

  if (!hubEnabled) {
    return <p className="text-[11px] text-muted-foreground">Accendi l’Agents HUB per scegliere un thread.</p>;
  }
  return (
    <div className="flex flex-wrap gap-2">
      {!channelLocked && (
        <ChannelSelect
          label={`${label}: canale`}
          channels={channels}
          value={channel}
          disabled={disabled}
          onChange={(next) => onChange(next, "")}
          className="h-9 w-auto min-w-[9rem] flex-1 text-sm"
        />
      )}
      <select
        aria-label={`${label}: thread`}
        value={threadId}
        disabled={disabled || threads === undefined}
        onChange={(e) => onChange(channel, e.target.value)}
        className={`${selectClass} min-w-[12rem] flex-[2]`}
      >
        <option value="">{threads === undefined ? "Caricamento…" : "Scegli un thread"}</option>
        {threads?.map((t) => (
          <option key={t.id} value={t.id}>
            {t.state === "open" ? t.title : `${t.title} (chiuso)`}
          </option>
        ))}
      </select>
    </div>
  );
}
