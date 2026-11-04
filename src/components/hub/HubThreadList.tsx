import { useMemo, useState } from "react";
import { Plus, SquareTerminal } from "lucide-react";
import { selectThread, setThreadFilter } from "../../app/actions/hub";
import { showWorkspaceTerminals } from "../../app/actions/view";
import { useStore } from "../../hooks/useStore";
import { formatRelative } from "../../lib/board";
import { GENERAL_CHANNEL, filterThreads } from "../../lib/hub";
import type { ThreadFilter } from "../../lib/hub";
import { cn } from "../../lib/utils";
import { hubStore } from "../../stores/hub";
import { useManagerNow } from "../tasks/useManagerNow";
import { Button } from "../ui/button";
import ListOwner from "./ListOwner";
import NewThreadForm from "./NewThreadForm";

const FILTERS: { id: ThreadFilter; label: string }[] = [
  { id: "open", label: "Aperti" },
  { id: "all", label: "Tutti" },
];

interface Props {
  channel: string;
  channelName: string;
}

/**
 * Colonna dei thread del canale: ai terminali del workspace (non nel Generale),
 * nuovo thread, filtro aperti/tutti, righe con stato e ultimo aggiornamento.
 */
export default function HubThreadList({ channel, channelName }: Props) {
  const all = useStore(hubStore, (s) => s.threads[channel]);
  const filter = useStore(hubStore, (s) => s.filter);
  const selectedId = useStore(hubStore, (s) => s.threadId);
  const [creating, setCreating] = useState(false);
  const now = useManagerNow();
  const shown = useMemo(() => filterThreads(all ?? [], filter), [all, filter]);

  return (
    <section aria-label="Thread" className="flex w-[280px] flex-shrink-0 flex-col border-r">
      <div className="flex items-center gap-2 border-b px-3 py-2">
        <h2 className="min-w-0 flex-1 truncate text-[13px] font-semibold">{channelName}</h2>
        {channel !== GENERAL_CHANNEL && (
          <Button
            type="button"
            size="sm"
            variant="outline"
            className="h-7 gap-1 px-2"
            onClick={() => showWorkspaceTerminals(channel)}
            title={`Vai ai terminali di ${channelName}`}
          >
            <SquareTerminal className="!size-3.5" aria-hidden="true" /> Terminali
          </Button>
        )}
        <Button
          type="button"
          size="icon"
          variant="outline"
          className="size-7"
          onClick={() => setCreating(true)}
          aria-label="Nuovo thread"
          title="Nuovo thread"
        >
          <Plus className="!size-3.5" />
        </Button>
      </div>
      {creating && <NewThreadForm key={channel} channel={channel} onDone={() => setCreating(false)} />}
      <div role="group" aria-label="Filtro" className="flex gap-1 border-b px-3 py-1.5">
        {FILTERS.map((f) => (
          <button
            key={f.id}
            type="button"
            aria-pressed={filter === f.id}
            onClick={() => setThreadFilter(f.id)}
            className={cn(
              "rounded px-2 py-0.5 text-[11px] transition-colors",
              filter === f.id ? "bg-accent text-foreground" : "text-muted-foreground hover:text-foreground",
            )}
          >
            {f.label}
          </button>
        ))}
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {all === undefined ? (
          <p className="p-3 text-xs text-muted-foreground">Caricamento…</p>
        ) : shown.length === 0 ? (
          <p className="p-3 text-xs text-muted-foreground">
            {all.length === 0
              ? "Nessuna attività in questo canale. Crea un thread per iniziare."
              : "Nessun thread aperto. Scegli “Tutti” per vedere quelli chiusi."}
          </p>
        ) : (
          <ul>
            {shown.map((t) => (
              <li key={t.id}>
                <button
                  type="button"
                  onClick={() => selectThread(channel, t.id)}
                  aria-current={t.id === selectedId ? "true" : undefined}
                  className={cn(
                    "flex w-full flex-col gap-0.5 border-b px-3 py-2 text-left transition-colors",
                    t.id === selectedId ? "bg-accent" : "hover:bg-accent/50",
                  )}
                >
                  <span className="flex items-center gap-2">
                    <span className="min-w-0 flex-1 truncate text-[13px] font-medium">{t.title}</span>
                    <ListOwner name={t.owner} />
                    {t.waiting_user && (
                      <span
                        className="h-2 w-2 flex-shrink-0 rounded-full bg-status-attention"
                        role="img"
                        aria-label="Aspetta una tua risposta"
                        title="Aspetta una tua risposta"
                      />
                    )}
                  </span>
                  <span className="flex items-center gap-2 text-[11px] text-muted-foreground">
                    <span>{t.state === "open" ? "Aperto" : "Chiuso"}</span>
                    <span aria-hidden="true">·</span>
                    <span>{formatRelative(t.updated_at, now)}</span>
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </section>
  );
}
