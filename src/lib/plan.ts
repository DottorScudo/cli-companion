// Task di pianificazione (`kind: "plan"`, docs/BOARD.MD › "Task di pianificazione"):
// etichetta, domande e risposta del pianificatore. Funzioni pure a zero dipendenze
// (solo tipi di board.ts), importabili sia dall'app sia dai test `node --test`.

import type { Task } from "./board.ts";

export const PLAN_LABEL = "Pianificazione";
export const PLAN_QUESTIONS_TITLE = "Domande del pianificatore";
export const PLAN_ANSWER_LABEL = "Risposta";

/** Come il pianificatore apre il riassunto quando chiude senza proposta (`planner_instructions.md`). */
const QUESTIONS_MARKER = /^questions for the user:?/i;
/** Come il pianificatore apre il riassunto quando risponde a una domanda invece di proporre. */
const ANSWER_MARKER = /^answer:/i;

type PlanOutcome = Pick<Task, "kind" | "status" | "report">;

/** Il task è di pianificazione; `kind` assente = work. */
export function isPlan(item: Pick<Task, "kind">): boolean {
  return item.kind === "plan";
}

/** Il testo dopo `marker` nel riassunto di un `plan` concluso; `null` se manca o è vuoto. */
function afterMarker(task: PlanOutcome, marker: RegExp): string | null {
  if (!isPlan(task) || task.status !== "done") return null;
  const summary = task.report?.summary.trim() ?? "";
  if (!marker.test(summary)) return null;
  const text = summary.replace(marker, "").trim();
  return text === "" ? null : text;
}

/**
 * Le domande con cui un `plan` concluso ha chiuso senza proposta (il testo dopo
 * «Questions for the user:»); `null` per gli altri task, per un piano senza domande
 * e per una risposta.
 */
export function planQuestions(task: PlanOutcome): string | null {
  return afterMarker(task, QUESTIONS_MARKER);
}

/**
 * La risposta con cui un `plan` concluso ha chiuso senza proposta (il testo dopo
 * «Answer:»); `null` per gli altri task e per un piano che ha proposto o chiesto.
 */
export function planAnswer(task: PlanOutcome): string | null {
  return afterMarker(task, ANSWER_MARKER);
}
