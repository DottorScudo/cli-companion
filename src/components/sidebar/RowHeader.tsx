import { memo } from "react";
import EnvChip from "../EnvChip";
import { cn } from "../../lib/utils";
import type { BuiltinLaunch, Launch, Workspace } from "../../lib/tauri";
import type { SidebarTerminal } from "../../stores/selectors";
import CollapsedInfo from "./CollapsedInfo";
import OpenFolderButton from "./OpenFolderButton";
import WorkspaceMenu from "./WorkspaceMenu";
import type { RowTaskCounts } from "./useWorkspaceRow";

interface Props {
  ws: Workspace;
  expanded: boolean;
  terminals: SidebarTerminal[];
  counts: RowTaskCounts | undefined;
  def: Launch | BuiltinLaunch | undefined;
}

function wsPath(ws: Workspace): string {
  return ws.kind === "wsl" && ws.distro ? `${ws.distro} · ${ws.path}` : ws.path;
}

/** Nome, ambiente e percorso del workspace, con le azioni a destra del nome. */
function RowHeader({ ws, expanded, terminals, counts, def }: Props) {
  return (
    <div className={cn("flex flex-col gap-0.5 px-2.5", expanded ? "pb-1 pt-1.5" : "py-2")}>
      <div className="flex min-h-6 items-center gap-2">
        <EnvChip ws={ws} />
        <span
          className={cn(
            "min-w-0 flex-1 truncate text-[13px]",
            expanded ? "font-semibold text-foreground" : "font-medium text-text-secondary",
          )}
        >
          {ws.name}
        </span>
        {expanded ? (
          <>
            <OpenFolderButton id={ws.id} />
            <WorkspaceMenu ws={ws} />
          </>
        ) : (
          <CollapsedInfo ws={ws} terminals={terminals} counts={counts} def={def} />
        )}
      </div>
      <p className="truncate pl-9 font-mono text-[11px] text-muted-foreground" title={wsPath(ws)}>
        {wsPath(ws)}
      </p>
    </div>
  );
}

export default memo(RowHeader);
