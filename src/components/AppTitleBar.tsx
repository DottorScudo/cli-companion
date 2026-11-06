import { useStore, useStores } from "../hooks/useStore";
import { useActiveWorld, useWorldRailShown } from "../hooks/useView";
import { selectGlobal, selectWorkspace } from "../app/actions/view";
import { layoutStore, shownLayout } from "../stores/layout";
import { sameTitleTabs, titleBarTabs } from "../stores/selectors";
import { runtimeStore } from "../stores/tabRuntime";
import { tabsStore } from "../stores/tabs";
import { workspacesStore } from "../stores/workspaces";
import TitleBar from "./TitleBar";

const TITLE_STORES = [layoutStore, tabsStore, runtimeStore, workspacesStore] as const;

const computeTitleTabs = () =>
  titleBarTabs(
    layoutStore.getState().layout,
    tabsStore.getState(),
    runtimeStore.getState(),
    workspacesStore.getState().list,
  );

/**
 * Barra del titolo: tab della vista globale (se ha tab) e di ogni gruppo, o
 * il percorso nel mondo Agents HUB. Il tab attivo è quello del layout mostrato
 * (con un workspace senza terminali a fuoco, nessuno). I tab si ricalcolano a ogni modifica di layout, tab o stato, ma la barra si
 * ridisegna solo se cambia qualcosa di visibile (un titolo OSC non basta).
 */
export default function AppTitleBar() {
  const { globalTab, groups } = useStores(TITLE_STORES, computeTitleTabs, sameTitleTabs);
  const globalActive = useStore(layoutStore, (s) => shownLayout(s).activeView === "global");
  const activeWorkspaceId = useStore(layoutStore, (s) => shownLayout(s).activeWorkspaceId);

  const world = useActiveWorld();
  const railShown = useWorldRailShown();

  return (
    <TitleBar
      world={world}
      railShown={railShown}
      globalTab={globalTab}
      globalActive={globalActive}
      groups={groups}
      activeWorkspaceId={activeWorkspaceId}
      onSelectGlobal={selectGlobal}
      onSelectGroup={selectWorkspace}
    />
  );
}
