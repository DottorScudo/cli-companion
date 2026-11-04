import { memo } from "react";
import { useStore, useStores } from "../../hooks/useStore";
import { closeOffice, focusTerminal } from "../../app/actions/view";
import { officeSession } from "../../lib/office";
import type { DeskMap, OfficeSession } from "../../lib/office";
import { shallowEqual } from "../../lib/store";
import { botImageResolver, configStore } from "../../stores/config";
import { tabLabel } from "../../stores/selectors";
import { runtimeStore } from "../../stores/tabRuntime";
import { tabsStore } from "../../stores/tabs";
import { visibleWorkspaces, workspaceById, workspacesStore } from "../../stores/workspaces";
import type { WorkspacesState } from "../../stores/workspaces";
import OfficeView from "../office/OfficeView";

const OFFICE_STORES = [tabsStore, runtimeStore, workspacesStore, configStore] as const;

/** Un bot per ogni tab aperto. */
function computeSessions(): OfficeSession[] {
  const tabs = tabsStore.getState();
  const runtime = runtimeStore.getState();
  const workspaces = workspacesStore.getState();
  const { botSettings, builtinLaunches } = configStore.getState();
  const imageOf = botImageResolver(botSettings);
  return tabs.map((tab) => {
    const rt = runtime[tab.key];
    const ws = workspaceById(workspaces, tab.workspaceId);
    return officeSession(
      {
        key: tab.key,
        bot: tab.bot,
        workspace: { id: tab.workspaceId, name: ws?.name ?? "" },
        launchId: tab.launchId,
        agent: rt?.agent,
        automation: rt?.automation,
      },
      rt?.busy === true,
      tabLabel(tab, rt, ws, builtinLaunches),
      imageOf(tab.bot),
    );
  });
}

/** Stesse sessioni? (campo per campo: gli oggetti si ricreano a ogni derivazione) */
const sameSessions = (a: OfficeSession[], b: OfficeSession[]) =>
  a.length === b.length && a.every((s, i) => shallowEqual(s, b[i]));

const workspaceOrder = (s: WorkspacesState) => visibleWorkspaces(s).map((w) => w.id);

interface Props {
  /** Scrivanie dei bot, tenute da Workbench per tutta la vita dei tab. */
  desks: DeskMap;
}

/**
 * L'Ufficio sta sopra tutto, Tasks Manager compreso (z-30 contro z-20);
 * anch'esso lascia i TerminalView montati al loro posto. Montato solo mentre
 * è aperto: i bot si costruiscono solo allora.
 */
function OfficeOverlay({ desks }: Props) {
  const sessions = useStores(OFFICE_STORES, computeSessions, sameSessions);
  const order = useStore(workspacesStore, workspaceOrder, shallowEqual);

  return (
    <div className="absolute inset-[3px] z-30 overflow-hidden rounded-[10px] border bg-pane-area">
      <OfficeView
        sessions={sessions}
        workspaceOrder={order}
        desks={desks}
        onOpenBot={focusTerminal}
        onClose={closeOffice}
      />
    </div>
  );
}

export default memo(OfficeOverlay);
