// Agents HUB: cosa mostra il pannello a destra di un thread — i suoi task (dalla
// board) e le sessioni dei loro worker ancora aperte. Funzioni pure a zero
// dipendenze, importabili sia dall'app sia dai test `node --test`.

import type { BoardView } from "../bindings/BoardView.ts";
import type { HubThread } from "../bindings/HubThread.ts";
import type { TaskView } from "../bindings/TaskView.ts";
import { GENERAL_CHANNEL } from "./hub.ts";

type Boards = Readonly<Record<string, Pick<BoardView, "tasks">>>;

/** Task del thread con il workspace della board in cui sta (serve ad aprirlo nel Tasks Manager). */
export interface ThreadPanelTask {
  task: TaskView;
  workspace: string;
}

function findTask(boards: Boards, taskId: string, channel: string): ThreadPanelTask | null {
  // Un thread di workspace ha i task nella board del suo canale; il Generale li cerca ovunque.
  const workspaces = channel === GENERAL_CHANNEL ? Object.keys(boards) : [channel];
  for (const workspace of workspaces) {
    const task = boards[workspace]?.tasks.find((t) => t.id === taskId);
    if (task) return { task, workspace };
  }
  return null;
}

/** Task del thread nell'ordine di `task_ids`; quelli non più nella board (eliminati) sono saltati. */
export function threadPanelTasks(thread: Pick<HubThread, "channel" | "task_ids">, boards: Boards): ThreadPanelTask[] {
  return thread.task_ids.flatMap((id) => {
    const found = findTask(boards, id, thread.channel);
    return found ? [found] : [];
  });
}

/** Chiavi dei tab dei worker dei task che sono ancora aperti, senza doppioni. */
export function threadPanelSessions(tasks: readonly ThreadPanelTask[], hasTab: (key: string) => boolean): string[] {
  const keys = tasks.flatMap(({ task }) => (task.session && hasTab(task.session) ? [task.session] : []));
  return [...new Set(keys)];
}

/** Stessi task (stessi oggetti, stesso workspace, stesso ordine)? Per non ridisegnare il pannello senza motivo. */
export function sameThreadPanelTasks(a: readonly ThreadPanelTask[], b: readonly ThreadPanelTask[]): boolean {
  return a.length === b.length && a.every((x, i) => x.task === b[i].task && x.workspace === b[i].workspace);
}
