import { memo } from "react";
import type { MouseEvent } from "react";
import { cn } from "../../lib/utils";
import type { Workspace } from "../../lib/tauri";
import { selectWorkspace } from "../../app/actions/view";
import { ArchiveConfirm, RemoveConfirm } from "./ConfirmBars";
import RenameForm from "./RenameForm";
import RowHeader from "./RowHeader";
import SpawnSplitButton from "./SpawnSplitButton";
import TasksButton from "./TasksButton";
import TerminalList from "./TerminalList";
import { sidebarStore } from "./sidebarState";
import { useDefaultLaunch, useRowManagerShown, useRowTaskCounts, useRowTerminals, useRowView } from "./useWorkspaceRow";
import { useWorkspaceReorder } from "./useWorkspaceReorder";

/** Click sulla riga (non sui suoi pulsanti): espande la card e seleziona il gruppo, se ha terminali. */
function selectOnClick(id: string) {
  return (e: MouseEvent<HTMLElement>) => {
    // I click su pulsanti, campi e menu (anche quelli nel portale, che
    // risalgono lungo l'albero React) non selezionano il gruppo.
    if ((e.target as HTMLElement).closest("button, input, a, [role^='menu']")) return;
    sidebarStore.expand(id);
    selectWorkspace(id);
  };
}

/**
 * Riga di un workspace: legge dagli archivi solo i propri terminali, task e
 * flag di interfaccia, quindi cambiano solo le righe che dipendono dal dato.
 */
function WorkspaceRow({ ws }: { ws: Workspace }) {
  const view = useRowView(ws.id);
  const terminals = useRowTerminals(ws.id);
  const counts = useRowTaskCounts(ws.id);
  const managerShown = useRowManagerShown(ws.id);
  const def = useDefaultLaunch(ws);
  const reorder = useWorkspaceReorder(ws.id);

  if (view.editing) return <RenameForm id={ws.id} busy={view.busy} />;

  return (
    <div
      role="group"
      aria-label={ws.name}
      {...reorder}
      onClick={selectOnClick(ws.id)}
      className={cn(
        "group relative cursor-pointer rounded-[10px] border transition-colors",
        view.dragging && "opacity-50",
        view.expanded
          ? "border-border bg-surface pb-1.5 pt-1"
          : "border-transparent hover:border-border hover:bg-surface/60",
      )}
    >
      {view.drop && (
        <span
          aria-hidden
          className={cn(
            "pointer-events-none absolute left-1 right-1 h-0.5 rounded bg-primary",
            view.drop === "before" ? "-top-[2px]" : "-bottom-[2px]",
          )}
        />
      )}
      <RowHeader ws={ws} expanded={view.expanded} terminals={terminals} counts={counts} def={def} />
      {view.confirming && <RemoveConfirm id={ws.id} busy={view.busy} />}
      {view.archiving && <ArchiveConfirm id={ws.id} name={ws.name} terminals={terminals.length} busy={view.busy} />}
      {view.expanded && (
        <>
          <TerminalList terminals={terminals} />
          <TasksButton id={ws.id} shown={managerShown} counts={counts} />
          <SpawnSplitButton ws={ws} def={def} />
        </>
      )}
    </div>
  );
}

export default memo(WorkspaceRow);
