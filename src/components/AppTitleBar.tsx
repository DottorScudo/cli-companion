import { useStore, useStores } from "../hooks/useStore";
import { selectGlobal, selectWorkspace } from "../app/actions/view";
import { layoutStore } from "../stores/layout";
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
 * Barra del titolo: tab della vista globale (se ha tab) e di ogni gruppo. I
 * tab si ricalcolano a ogni modifica di layout, tab o stato, ma la barra si
 * ridisegna solo se cambia qualcosa di visibile (un titolo OSC non basta).
 */
export default function AppTitleBar() {
  const { globalTab, groups } = useStores(TITLE_STORES, computeTitleTabs, sameTitleTabs);
  const globalActive = useStore(layoutStore, (s) => s.layout.activeView === "global");
  const activeWorkspaceId = useStore(layoutStore, (s) => s.layout.activeWorkspaceId);

  return (
    <TitleBar
      globalTab={globalTab}
      globalActive={globalActive}
      groups={groups}
      activeWorkspaceId={activeWorkspaceId}
      onSelectGlobal={selectGlobal}
      onSelectGroup={selectWorkspace}
    />
  );
}
