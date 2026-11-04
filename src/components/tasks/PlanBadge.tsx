import { ClipboardList } from "lucide-react";
import { cn } from "../../lib/utils";
import { PLAN_LABEL } from "../../lib/plan";

/** Badge "Pianificazione": distingue i task che esplorano il repository e propongono i task da quelli di lavoro. */
export function PlanBadge({ className }: { className?: string }) {
  return (
    <span
      className={cn(
        "inline-flex flex-shrink-0 items-center gap-1 rounded border border-primary/40 px-1.5 py-px font-medium text-primary",
        className,
      )}
    >
      <ClipboardList className="h-3 w-3" aria-hidden="true" />
      {PLAN_LABEL}
    </span>
  );
}
