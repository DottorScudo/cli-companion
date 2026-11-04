import { useCallback } from "react";
import type { BuiltinLaunch, Launch, Workspace } from "../../lib/tauri";
import type { TaskCounts } from "../../lib/board";
import { shallowEqual } from "../../lib/store";
import { resolveDefaultLaunch } from "../../lib/launch";
import { useStore, useStores } from "../../hooks/useStore";
import { boardsStore, taskCountsOf } from "../../stores/boards";
import type { BoardsState } from "../../stores/boards";
import { botImageResolver, configStore, launchesFor } from "../../stores/config";
import { layoutStore, viewWorkspaceId } from "../../stores/layout";
import { sameSidebarTerminals, sidebarTerminals } from "../../stores/selectors";
import type { SidebarTerminal } from "../../stores/selectors";
import { runtimeStore } from "../../stores/tabRuntime";
import { tabsStore } from "../../stores/tabs";
import { uiStore } from "../../stores/ui";
import { workspacesStore } from "../../stores/workspaces";
import { rowView, sidebarStore } from "./sidebarState";
import type { SidebarState } from "./sidebarState";

// Selezioni di una riga: ciascuna legge solo ciò che le serve e restituisce
// un valore stabile finché quello non cambia, così la riga non si ridisegna
// per i cambiamenti delle altre.

const TERMINAL_STORES = [layoutStore, tabsStore, runtimeStore, workspacesStore, configStore] as const;
const MANAGER_STORES = [layoutStore, uiStore] as const;

export type RowTaskCounts = Pick<TaskCounts, "running" | "queued">;

const sameCounts = (a: RowTaskCounts | undefined, b: RowTaskCounts | undefined): boolean =>
  a?.running === b?.running && a?.queued === b?.queued;

/** Flag di interfaccia della riga (rinomina, conferme, espansione, trascinamento). */
export function useRowView(id: string) {
  const select = useCallback((s: SidebarState) => rowView(s, id), [id]);
  return useStore(sidebarStore, select, shallowEqual);
}

/** Terminali del workspace nell'ordine della griglia. */
export function useRowTerminals(id: string): SidebarTerminal[] {
  const compute = useCallback(() => {
    const { builtinLaunches, botSettings } = configStore.getState();
    return sidebarTerminals(
      id,
      layoutStore.getState().layout,
      tabsStore.getState(),
      runtimeStore.getState(),
      workspacesStore.getState().list,
      builtinLaunches,
      botSettings,
      botImageResolver(botSettings),
    );
  }, [id]);
  return useStores(TERMINAL_STORES, compute, sameSidebarTerminals);
}

/** Task in corso / in coda del workspace (dal suo `BoardView`). */
export function useRowTaskCounts(id: string): RowTaskCounts | undefined {
  const select = useCallback((s: BoardsState) => taskCountsOf(s.boards)[id], [id]);
  return useStore(boardsStore, select, sameCounts);
}

/** Il Tasks Manager di questo workspace è mostrato al posto dei terminali. */
export function useRowManagerShown(id: string): boolean {
  const compute = useCallback(
    () => viewWorkspaceId(layoutStore.getState()) === id && uiStore.getState().managerOpen[id] === true,
    [id],
  );
  return useStores(MANAGER_STORES, compute);
}

/** Avvio predefinito del workspace (proprio, altrimenti globale); undefined = shell vuota. */
export function useDefaultLaunch(ws: Workspace): Launch | BuiltinLaunch | undefined {
  const builtins = useStore(configStore, (s) => launchesFor(s, ws));
  const globalLaunch = useStore(configStore, (s) => s.globalLaunch);
  return resolveDefaultLaunch(ws, builtins, globalLaunch);
}
