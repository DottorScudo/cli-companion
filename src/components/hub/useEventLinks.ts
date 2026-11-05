import { useCallback } from "react";
import { useStores } from "../../hooks/useStore";
import { workerTabOf } from "../../lib/hubEvents";
import { eventTaskOf } from "../../lib/hubProposals";
import type { Task } from "../../lib/board";
import type { HubEvent } from "../../lib/tauri";
import { boardsStore } from "../../stores/boards";
import { tabsStore } from "../../stores/tabs";

const WORKER_STORES = [boardsStore, tabsStore] as const;
const BOARDS_ONLY = [boardsStore] as const;

/** Dove portano i link di un evento: il tab aperto del suo worker e il suo task nella board (se esistono ancora). */
export function useEventLinks(event: HubEvent): { workerTab: string | null; task: Task | null } {
  const computeWorkerTab = useCallback(
    () => workerTabOf(boardsStore.getState().boards, event, (key) => tabsStore.get(key) !== undefined),
    [event],
  );
  const workerTab = useStores(WORKER_STORES, computeWorkerTab);
  const computeTask = useCallback(() => eventTaskOf(boardsStore.getState().boards, event), [event]);
  const task = useStores(BOARDS_ONLY, computeTask);
  return { workerTab, task };
}
