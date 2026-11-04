import { useId, useState } from "react";
import type { ReactNode } from "react";
import { ChevronRight } from "lucide-react";
import { cn } from "../../../lib/utils";

interface Props {
  title: string;
  description: string;
  /** Riepilogo a destra dell'intestazione, visibile anche da chiuso. */
  summary: string;
  defaultOpen?: boolean;
  children: ReactNode;
}

/** Sezione ripiegabile: l'intestazione è un pulsante (`aria-expanded`) collegato al suo contenuto. */
export default function HubAccordion({ title, description, summary, defaultOpen = false, children }: Props) {
  const [open, setOpen] = useState(defaultOpen);
  const baseId = useId();
  const buttonId = `${baseId}-button`;
  const panelId = `${baseId}-panel`;

  return (
    <section className="overflow-hidden rounded-lg border bg-card">
      <h3>
        <button
          type="button"
          id={buttonId}
          aria-expanded={open}
          aria-controls={panelId}
          onClick={() => setOpen((v) => !v)}
          className="flex w-full items-center gap-3 px-3 py-3 text-left transition-colors hover:bg-surface/40 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-ring"
        >
          <ChevronRight
            aria-hidden="true"
            className={cn("!size-4 flex-shrink-0 text-muted-foreground transition-transform", open && "rotate-90")}
          />
          <span className="flex min-w-0 flex-1 flex-col gap-0.5">
            <span className="text-[13px] font-semibold">{title}</span>
            <span className="text-[11px] font-normal text-muted-foreground">{description}</span>
          </span>
          <span className="max-w-[45%] flex-shrink-0 truncate text-[11px] text-text-secondary" title={summary}>
            {summary}
          </span>
        </button>
      </h3>
      <div
        id={panelId}
        role="region"
        aria-labelledby={buttonId}
        hidden={!open}
        className="flex flex-col gap-3 border-t p-3"
      >
        {children}
      </div>
    </section>
  );
}
