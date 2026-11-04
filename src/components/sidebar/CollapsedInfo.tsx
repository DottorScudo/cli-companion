import { memo } from "react";
import { Plus } from "lucide-react";
import StatusIcon from "../StatusIcon";
import { GROUP_STATUS } from "../../lib/agents";
import { groupIndicator } from "../../lib/layout";
import { launchLabel } from "../../lib/launchMenu";
import type { BuiltinLaunch, Launch, Workspace } from "../../lib/tauri";
import { spawnTab } from "../../app/actions/tabs";
import type { SidebarTerminal } from "../../stores/selectors";
import OpenFolderButton from "./OpenFolderButton";
import WorkspaceMenu from "./WorkspaceMenu";
import { ICON_BTN } from "./styles";
import type { RowTaskCounts } from "./useWorkspaceRow";

interface Props {
  ws: Workspace;
  terminals: SidebarTerminal[];
  counts: RowTaskCounts | undefined;
  def: Launch | BuiltinLaunch | undefined;
}

/** Riga chiusa: stato del gruppo, task in corso, numero di terminali e azioni (visibili al passaggio del mouse). */
function CollapsedInfo({ ws, terminals, counts, def }: Props) {
  const indicator = groupIndicator(terminals.map((t) => t.status));
  const running = counts?.running ?? 0;
  return (
    <>
      {indicator !== "none" && <StatusIcon status={GROUP_STATUS[indicator]} />}
      {running > 0 && (
        <span
          className="rounded bg-status-running/15 px-1 font-mono text-[10px] text-status-running"
          title={`Tasks: ${running} in corso`}
        >
          {running}
        </span>
      )}
      {terminals.length > 0 && (
        <span className="font-mono text-[11px] text-faint group-focus-within:hidden group-hover:hidden group-has-[[data-state=open]]:hidden">
          {terminals.length}
        </span>
      )}
      {/* Azioni visibili solo con il mouse sopra (o il focus dentro). Con un
          menu aperto restano visibili: il menu è non modale, il focus passa
          al portale e il mouse esce dalla riga, ma se il trigger sparisse
          (display: none) l'ancora avrebbe un rettangolo nullo e Radix
          riposizionerebbe il menu in cima alla finestra. */}
      <span className="hidden items-center gap-0.5 group-focus-within:flex group-hover:flex has-[[data-state=open]]:flex">
        <button
          type="button"
          className={ICON_BTN}
          title={def ? `Nuovo terminale: ${launchLabel(def)}` : "Nuovo terminale"}
          aria-label="Nuovo terminale"
          onClick={() => spawnTab(ws)}
        >
          <Plus />
        </button>
        <OpenFolderButton id={ws.id} />
        <WorkspaceMenu ws={ws} />
      </span>
    </>
  );
}

export default memo(CollapsedInfo);
