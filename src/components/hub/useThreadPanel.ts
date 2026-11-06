import { useCallback } from "react";
import { useStores } from "../../hooks/useStore";
import { sameThreadPanelTasks, threadPanelSessions, threadPanelTasks } from "../../lib/hubThreadPanel";
import type { ThreadPanelTask } from "../../lib/hubThreadPanel";
import type { HubThread } from "../../lib/tauri";
import { boardsStore } from "../../stores/boards";
import { botImageResolver, configStore } from "../../stores/config";
import { layoutStore } from "../../stores/layout";
import { sameSidebarTerminals, terminalsByKey } from "../../stores/selectors";
import type { SidebarTerminal } from "../../stores/selectors";
import { runtimeStore } from "../../stores/tabRuntime";
import { tabsStore } from "../../stores/tabs";
import { workspacesStore } from "../../stores/workspaces";

const BOARD_STORES = [boardsStore] as const;
const TERMINAL_STORES = [boardsStore, layoutStore, tabsStore, runtimeStore, workspacesStore, configStore] as const;

type PanelThread = Pick<HubThread, "channel" | "task_ids">;

/** Task del thread presenti in una board, con stato sempre aggiornato. */
export function useThreadPanelTasks(thread: PanelThread): ThreadPanelTask[] {
  const compute = useCallback(() => threadPanelTasks(thread, boardsStore.getState().boards), [thread]);
  return useStores(BOARD_STORES, compute, sameThreadPanelTasks);
}

/** Terminali ancora aperti dei worker dei task del thread, con etichetta e stato live. */
export function useThreadPanelTerminals(thread: PanelThread): SidebarTerminal[] {
  const compute = useCallback(() => {
    const tasks = threadPanelTasks(thread, boardsStore.getState().boards);
    const keys = threadPanelSessions(tasks, (key) => tabsStore.get(key) !== undefined);
    const { builtinLaunches, botSettings } = configStore.getState();
    return terminalsByKey(
      keys,
      layoutStore.getState().layout,
      tabsStore.getState(),
      runtimeStore.getState(),
      workspacesStore.getState().list,
      builtinLaunches,
      botSettings,
      botImageResolver(botSettings),
    );
  }, [thread]);
  return useStores(TERMINAL_STORES, compute, sameSidebarTerminals);
}
