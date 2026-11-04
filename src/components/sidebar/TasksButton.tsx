import { memo } from "react";
import { ListChecks } from "lucide-react";
import { cn } from "../../lib/utils";
import { toggleManager } from "../../app/actions/view";
import TaskCountsLabel from "./TaskCountsLabel";
import type { RowTaskCounts } from "./useWorkspaceRow";

interface Props {
  id: string;
  /** Il Tasks Manager del workspace è mostrato al posto dei terminali. */
  shown: boolean;
  counts: RowTaskCounts | undefined;
}

/** Tasks Manager del workspace (Ctrl+Maiusc+B): in corso · in coda. */
function TasksButton({ id, shown, counts }: Props) {
  return (
    <div className="mx-1.5 mt-1.5">
      <button
        type="button"
        aria-pressed={shown}
        title={shown ? "Torna ai terminali (Ctrl+Maiusc+B)" : "Apri il Tasks Manager (Ctrl+Maiusc+B)"}
        onClick={() => toggleManager(id)}
        className={cn(
          "flex w-full items-center gap-2 rounded-md border px-2.5 py-1.5 transition-colors",
          shown
            ? "border-primary/50 bg-surface-2 text-foreground"
            : "border-border-strong text-muted-foreground hover:border-muted-foreground hover:text-foreground",
        )}
      >
        <ListChecks className="h-3 w-3 flex-shrink-0" />
        <span className="text-xs">Tasks</span>
        <TaskCountsLabel counts={counts} />
      </button>
    </div>
  );
}

export default memo(TasksButton);
