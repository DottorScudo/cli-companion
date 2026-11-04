// Task di pianificazione (`kind: "plan"`, docs/BOARD.MD › "Task di pianificazione"):
// etichetta e domande del pianificatore. Funzioni pure a zero dipendenze (solo
// tipi di board.ts), importabili sia dall'app sia dai test `node --test`.

import type { Task } from "./board.ts";

export const PLAN_LABEL = "Pianificazione";
export const PLAN_QUESTIONS_TITLE = "Domande del pianificatore";

/** Come il pianificatore apre il riassunto quando chiude senza proposta (`planner_instructions.md`). */
const QUESTIONS_MARKER = /^questions for the user:?/i;

/** Il task è di pianificazione; `kind` assente = work. */
export function isPlan(item: Pick<Task, "kind">): boolean {
  return item.kind === "plan";
}

/**
 * Le domande con cui un `plan` concluso ha chiuso senza proposta (il testo dopo
 * «Questions for the user:»); `null` per gli altri task e per un piano senza domande.
 */
export function planQuestions(task: Pick<Task, "kind" | "status" | "report">): string | null {
  if (!isPlan(task) || task.status !== "done") return null;
  const summary = task.report?.summary.trim() ?? "";
  if (!QUESTIONS_MARKER.test(summary)) return null;
  const questions = summary.replace(QUESTIONS_MARKER, "").trim();
  return questions === "" ? null : questions;
}
