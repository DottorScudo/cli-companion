import { useId, useState } from "react";
import { ChevronRight } from "lucide-react";
import type { TaskSpec } from "../../lib/board";
import { isReview } from "../../lib/review";
import { cn } from "../../lib/utils";
import { ReviewBadge } from "../tasks/ReviewBadges";
import Markdown from "../tasks/Markdown";

interface Props {
  spec: TaskSpec;
  /** Dipendenze con il titolo, se noto (altrimenti l'id). */
  dependencies: readonly string[];
}

/** Un task della proposta: id, titolo, modello e dipendenze; si espande per leggere la specifica completa. */
export default function HubProposalTask({ spec, dependencies }: Props) {
  const [open, setOpen] = useState(false);
  const panelId = useId();
  return (
    <li className="min-w-0 rounded-md border bg-card">
      <button
        type="button"
        aria-expanded={open}
        aria-controls={panelId}
        onClick={() => setOpen((v) => !v)}
        className="flex w-full min-w-0 items-start gap-1.5 px-2 py-1.5 text-left focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
      >
        <ChevronRight
          className={cn("mt-0.5 !size-3.5 flex-shrink-0 transition-transform", open && "rotate-90")}
          aria-hidden="true"
        />
        <span className="flex min-w-0 flex-1 flex-col gap-0.5">
          <span className="break-words text-xs font-medium">
            {spec.title}
            {isReview(spec) && <ReviewBadge className="ml-1.5 align-middle text-[10px]" />}
          </span>
          <span className="flex flex-wrap items-baseline gap-x-1.5 text-[11px] text-muted-foreground">
            <span className="font-mono">{spec.id}</span>
            {spec.model && <span>· {spec.model}</span>}
            {dependencies.length > 0 && <span className="break-words">· dipende da {dependencies.join(", ")}</span>}
          </span>
        </span>
      </button>
      {open && (
        <div id={panelId} className="border-t px-3 py-2">
          <Markdown source={spec.task} />
        </div>
      )}
    </li>
  );
}
