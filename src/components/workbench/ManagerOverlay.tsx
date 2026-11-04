import { memo, useEffect } from "react";
import { useStore } from "../../hooks/useStore";
import { useManagerShown, useViewWorkspaceId } from "../../hooks/useView";
import { openIntake } from "../../app/actions/tabs";
import { focusTerminal, toggleManager } from "../../app/actions/view";
import { storeBoard } from "../../app/boards";
import { boardsStore } from "../../stores/boards";
import { tabsStore } from "../../stores/tabs";
import { uiStore } from "../../stores/ui";
import { visibleWorkspaces, workspacesStore } from "../../stores/workspaces";
import TasksManager from "../tasks/TasksManager";

/** Il tab del worker esiste ancora? (serve a "Apri terminale" di un task Fallito) */
const isTabOpen = (id: string): boolean => tabsStore.get(id) !== undefined;

/**
 * Il Tasks Manager copre pannello Task e area dei pannelli senza toccarli: i
 * TerminalView restano montati e al loro posto.
 */
function ManagerOverlay() {
  const viewId = useViewWorkspaceId();
  const shown = useManagerShown();
  const intent = useStore(uiStore, (s) => s.managerIntent);
  const visible = useStore(workspacesStore, visibleWorkspaces);
  const board = useStore(boardsStore, (s) => (viewId ? s.boards[viewId] : undefined));
  const loadError = useStore(boardsStore, (s) => (viewId ? s.errors[viewId] : undefined));

  // La richiesta di apertura vale una volta: il manager l'ha letta al montaggio
  // (gli effect dei figli girano prima), poi si azzera, così una riapertura
  // senza richiesta non riseleziona un task vecchio o riapre "Nuovo task".
  useEffect(() => {
    if (shown && intent) uiStore.consumeIntent();
  }, [shown, intent]);

  const workspace = shown ? visible.find((w) => w.id === viewId) : undefined;
  if (!workspace) return null;

  return (
    <div className="absolute inset-[3px] z-20 overflow-hidden rounded-[10px] border">
      <TasksManager
        key={workspace.id}
        workspace={workspace}
        board={board ?? null}
        loadError={loadError ?? null}
        onBoard={storeBoard}
        onBackToTerminals={() => toggleManager(workspace.id)}
        onOpenTerminal={focusTerminal}
        onOpenIntake={() => openIntake(workspace.id)}
        intent={intent}
        isTabOpen={isTabOpen}
      />
    </div>
  );
}

export default memo(ManagerOverlay);
