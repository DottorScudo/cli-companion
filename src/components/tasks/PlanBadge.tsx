import { ClipboardList, MessageSquareText } from "lucide-react";
import { cn } from "../../lib/utils";
import { PLAN_ANSWER_LABEL, PLAN_LABEL } from "../../lib/plan";

const BADGE_BASE = "inline-flex flex-shrink-0 items-center gap-1 rounded border px-1.5 py-px font-medium";

/** Badge "Pianificazione": distingue i task che esplorano il repository e propongono i task da quelli di lavoro. */
export function PlanBadge({ className }: { className?: string }) {
  return (
    <span className={cn(BADGE_BASE, "border-primary/40 text-primary", className)}>
      <ClipboardList className="h-3 w-3" aria-hidden="true" />
      {PLAN_LABEL}
    </span>
  );
}

/** Badge "Risposta": il pianificatore ha risposto a una domanda invece di proporre task. */
export function PlanAnswerBadge({ className }: { className?: string }) {
  return (
    <span className={cn(BADGE_BASE, "border-status-finished/40 text-status-finished", className)}>
      <MessageSquareText className="h-3 w-3" aria-hidden="true" />
      {PLAN_ANSWER_LABEL}
    </span>
  );
}
