import { useId, useState } from "react";
import { ChevronRight } from "lucide-react";
import type { EventReport } from "../../lib/hubEvents";
import { cn } from "../../lib/utils";
import Markdown from "../tasks/Markdown";

interface Props {
  report: EventReport;
  /** Il task è fallito: la nota si mostra con il tono di errore, come nel dettaglio del task. */
  failed: boolean;
}

/** Avviso di testo accorciato, se c'è. */
function CutNotice({ text }: { text: string | null }) {
  if (text === null) return null;
  return <p className="text-[11px] italic text-muted-foreground">{text}</p>;
}

/**
 * Report di un task concluso sotto la riga dell'evento: chiuso di default, si apre con «Mostra report» e mostra il
 * riepilogo (markdown) e la nota in un'area con altezza massima e scroll interno.
 */
export default function HubEventReport({ report, failed }: Props) {
  const [open, setOpen] = useState(false);
  const panelId = useId();
  const { summary, note } = report;
  return (
    <div className="flex min-w-0 flex-col gap-1">
      <button
        type="button"
        aria-expanded={open}
        aria-controls={panelId}
        onClick={() => setOpen((v) => !v)}
        className="flex items-center gap-1 self-start text-[11px] text-primary hover:underline focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
      >
        <ChevronRight className={cn("!size-3 transition-transform", open && "rotate-90")} aria-hidden="true" />
        {open ? "Nascondi report" : "Mostra report"}
      </button>
      {open && (
        <div
          id={panelId}
          className="flex max-h-80 min-w-0 flex-col gap-2 overflow-y-auto rounded-md border bg-surface px-3 py-2"
        >
          <Markdown source={summary.text} />
          <CutNotice text={summary.cutNotice} />
          {note && (
            <div className="flex flex-col gap-1 border-t pt-2">
              <p className="text-[11px] font-semibold text-muted-foreground">Nota</p>
              <p
                className={cn(
                  "whitespace-pre-line break-words text-xs leading-relaxed",
                  failed ? "text-destructive" : "text-text-secondary",
                )}
              >
                {note.text}
              </p>
              <CutNotice text={note.cutNotice} />
            </div>
          )}
        </div>
      )}
    </div>
  );
}
