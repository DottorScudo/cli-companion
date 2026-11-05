import { useEffect, useMemo, useState } from "react";
import { Plus, Search } from "lucide-react";
import { setThreadFilter } from "../../app/actions/hub";
import { loadChatPreviews } from "../../app/actions/hubChatState";
import { useHubChannels } from "../../hooks/useHubChannels";
import { useStore } from "../../hooks/useStore";
import { GENERAL_CHANNEL } from "../../lib/hub";
import type { ThreadFilter } from "../../lib/hub";
import { chatList } from "../../lib/hubChats";
import { isUnread } from "../../lib/hubUnread";
import type { HubSecretary } from "../../lib/tauri";
import { hubStore } from "../../stores/hub";
import { useManagerNow } from "../tasks/useManagerNow";
import { Button } from "../ui/button";
import { Input } from "../ui/input";
import ChannelSelect from "./ChannelSelect";
import HubChatCard from "./HubChatCard";
import HubSegmented from "./HubSegmented";
import NewThreadForm from "./NewThreadForm";

const FILTERS: { id: ThreadFilter; label: string }[] = [
  { id: "open", label: "Aperti" },
  { id: "all", label: "Tutti" },
];

/** Valore del selettore del workspace che vale «tutti». */
const ALL_CHANNELS = "";

const NO_SECRETARIES: readonly HubSecretary[] = [];

function EmptyNote({ children }: { children: string }) {
  return <p className="px-4 py-3 text-xs text-muted-foreground">{children}</p>;
}

/**
 * Elenco delle chat della vista messenger: i thread di tutti i canali visibili,
 * dal più recente, con ricerca, filtro Aperti/Tutti e per workspace, e il «+»
 * per un nuovo thread nel workspace scelto.
 */
export default function HubChatList() {
  const { channels, current } = useHubChannels();
  const threads = useStore(hubStore, (s) => s.threads);
  const filter = useStore(hubStore, (s) => s.filter);
  const previews = useStore(hubStore, (s) => s.previews);
  const lastSeen = useStore(hubStore, (s) => s.lastSeen);
  const selectedId = useStore(hubStore, (s) => s.threadId);
  const secretaries = useStore(hubStore, (s) => s.settings?.secretaries) ?? NO_SECRETARIES;
  const [query, setQuery] = useState("");
  const [workspace, setWorkspace] = useState(ALL_CHANNELS);
  const [creating, setCreating] = useState(false);
  const now = useManagerNow();

  // Un workspace filtrato che non c'è più (rimosso o archiviato) equivale a tutti.
  const onlyChannel = channels.some((c) => c.id === workspace) ? workspace : null;
  const everything = useMemo(() => chatList(threads, channels, "all", ""), [threads, channels]);
  const shown = useMemo(
    () => chatList(threads, channels, filter, query, onlyChannel),
    [threads, channels, filter, query, onlyChannel],
  );

  useEffect(() => {
    void loadChatPreviews(shown.map((e) => e.thread));
  }, [shown]);

  return (
    <section aria-label="Elenco chat" className="flex min-h-0 flex-[3] flex-col">
      <div className="flex items-center gap-2 px-3 pb-2 pt-3">
        <div className="relative min-w-0 flex-1">
          <Search
            className="pointer-events-none absolute left-2 top-1/2 !size-3.5 -translate-y-1/2 text-muted-foreground"
            aria-hidden="true"
          />
          <Input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Cerca chat…"
            aria-label="Cerca chat"
            className="h-8 pl-7 text-[13px]"
          />
        </div>
        <Button
          type="button"
          size="icon"
          variant="outline"
          className="size-8"
          onClick={() => setCreating(true)}
          aria-label="Nuovo thread"
          title="Nuovo thread"
        >
          <Plus className="!size-3.5" />
        </Button>
      </div>
      {creating && (
        <NewThreadForm
          channel={selectedId === null ? GENERAL_CHANNEL : current}
          channels={channels}
          onDone={() => setCreating(false)}
        />
      )}
      <div className="flex items-center gap-2 border-b px-3 pb-2">
        <HubSegmented label="Filtro" options={FILTERS} value={filter} onChange={setThreadFilter} />
        <ChannelSelect
          label="Filtra per workspace"
          channels={channels}
          value={onlyChannel ?? ALL_CHANNELS}
          allLabel="Tutti i workspace"
          onChange={setWorkspace}
          className="min-w-0 flex-1"
        />
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {everything.length === 0 ? (
          <EmptyNote>Nessuna chat. Crea un thread con +</EmptyNote>
        ) : shown.length === 0 ? (
          <EmptyNote>Nessuna chat corrisponde alla ricerca o ai filtri.</EmptyNote>
        ) : (
          <ul aria-label="Chat">
            {shown.map((entry) => (
              <HubChatCard
                key={entry.thread.id}
                entry={entry}
                preview={previews[entry.thread.id]}
                unread={isUnread(lastSeen, entry.thread)}
                selected={entry.thread.id === selectedId && entry.thread.channel === current}
                now={now}
                secretaries={secretaries}
              />
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
