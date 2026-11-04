// Cosa l'interfaccia propone su un task: legge i campi che il backend calcola
// (`allowed_actions`, `cascade`; `can_edit` si legge direttamente) e non
// ricalcola la tabella degli stati. Funzioni pure, importabili anche dai test.

import type { Task, TaskAction } from "./board.ts";

/** Il backend consente `action` su questo task adesso? */
export function canDo(task: Pick<Task, "allowed_actions">, action: TaskAction): boolean {
  return task.allowed_actions.includes(action);
}

/** Come si presenta "Elimina" su un task. */
export type DeleteControl =
  /** Si elimina il solo task (`delete`) o lui e i dipendenti (`delete_cascade`). */
  | { kind: "delete" | "delete_cascade" }
  /** I dipendenti in corso (`running`) vanno fermati prima: il pulsante resta, disattivato. */
  | { kind: "blocked"; running: string[] };

/** Il pulsante "Elimina" del task; `null` se non si può eliminare. */
export function deleteControl(task: Pick<Task, "allowed_actions" | "cascade">): DeleteControl | null {
  if (canDo(task, "delete_cascade")) return { kind: "delete_cascade" };
  if (canDo(task, "delete")) return { kind: "delete" };
  return task.cascade.running.length > 0 ? { kind: "blocked", running: task.cascade.running } : null;
}
