// Resa dei messaggi evento dell'Agents HUB: etichetta, icona e tono per tipo.
// Il backend può aggiungere tipi (compatibile in lettura): uno sconosciuto si
// mostra con la resa generica, mai come errore.

import type { BoardView } from "../bindings/BoardView.ts";
import type { HubEvent } from "../bindings/HubEvent.ts";
import type { TaskView } from "../bindings/TaskView.ts";

/** Chiave dell'icona; il componente la traduce in un'icona. */
export type HubEventIcon =
  "proposal" | "started" | "done" | "failed" | "attention" | "question" | "deleted" | "request" | "generic";

export type HubEventTone = "neutral" | "success" | "danger" | "warning";

export interface HubEventLook {
  label: string;
  icon: HubEventIcon;
  tone: HubEventTone;
}

const GENERIC_LOOK: HubEventLook = { label: "Evento", icon: "generic", tone: "neutral" };

const EVENT_LOOKS: Readonly<Record<string, HubEventLook>> = {
  proposal_created: { label: "Proposta", icon: "proposal", tone: "warning" },
  proposal_accepted: { label: "Proposta accettata", icon: "proposal", tone: "success" },
  proposal_rejected: { label: "Proposta rifiutata", icon: "proposal", tone: "neutral" },
  task_created: { label: "Creato", icon: "generic", tone: "neutral" },
  task_started: { label: "Avviato", icon: "started", tone: "neutral" },
  task_done: { label: "Completato", icon: "done", tone: "success" },
  task_failed: { label: "Fallito", icon: "failed", tone: "danger" },
  task_attention: { label: "Attenzione", icon: "attention", tone: "warning" },
  task_question: { label: "Domanda", icon: "question", tone: "warning" },
  task_question_answered: { label: "Risposta", icon: "question", tone: "neutral" },
  task_deleted: { label: "Eliminato", icon: "deleted", tone: "neutral" },
  action_request: { label: "Richiesta", icon: "request", tone: "warning" },
  action_request_resolved: { label: "Esito richiesta", icon: "request", tone: "neutral" },
};

/** Resa di un tipo di evento (anche uno che questa versione non conosce). */
export function eventLook(kind: string): HubEventLook {
  return EVENT_LOOKS[kind] ?? GENERIC_LOOK;
}

/**
 * Tab del terminale del worker del task di un evento: la `session` del task
 * nella board del workspace, se il tab è ancora aperto (`hasTab`); altrimenti `null`.
 */
export function workerTabOf(
  boards: Readonly<Record<string, Pick<BoardView, "tasks">>>,
  event: Pick<HubEvent, "workspace" | "task_id">,
  hasTab: (key: string) => boolean,
): string | null {
  if (event.workspace === null || event.task_id === null) return null;
  const task = boards[event.workspace]?.tasks.find((t) => t.id === event.task_id);
  return task?.session && hasTab(task.session) ? task.session : null;
}

/** Tipi di evento di un task concluso (finito, revisione finita, fallito): hanno un report da leggere. */
const REPORT_EVENT_KINDS: ReadonlySet<string> = new Set(["task_done", "task_failed"]);

/** Un testo del report da mostrare: `cutNotice` dice se è accorciato e quanto era lungo. */
export interface EventReportText {
  text: string;
  cutNotice: string | null;
}

/** Report di un evento di task concluso: riepilogo (sempre) e nota (se c'è). */
export interface EventReport {
  summary: EventReportText;
  note: EventReportText | null;
}

/** Avviso per un testo dell'evento accorciato, di cui il testo intero non è più disponibile. */
export function cutNotice(fullChars: number, taskInBoard: boolean): string {
  const where = taskInBoard ? "il report intero non è più nel task" : "il task non è più nella board";
  return `Testo accorciato: ${fullChars} caratteri in totale, ${where}`;
}

function hasText(text: string | null | undefined): text is string {
  return typeof text === "string" && text.trim() !== "";
}

/** Il testo intero dalla board se c'è, altrimenti quello (forse accorciato) dell'evento. */
function pickText(
  full: string | null | undefined,
  short: string | null,
  shortChars: number | undefined,
  taskInBoard: boolean,
): EventReportText | null {
  if (hasText(full)) return { text: full, cutNotice: null };
  if (!hasText(short)) return null;
  return { text: short, cutNotice: shortChars === undefined ? null : cutNotice(shortChars, taskInBoard) };
}

/**
 * Report da mostrare per un evento di task concluso: il riepilogo e la nota interi presi dal task nella board
 * (`task`, `null` se non c'è più), altrimenti quelli dell'evento con l'avviso se erano accorciati. `null` per gli
 * altri tipi di evento e per gli eventi senza riepilogo.
 */
export function eventReport(
  event: Pick<HubEvent, "kind" | "summary" | "summary_chars" | "note" | "note_chars">,
  task: Pick<TaskView, "report" | "note"> | null,
): EventReport | null {
  if (!REPORT_EVENT_KINDS.has(event.kind)) return null;
  const inBoard = task !== null;
  const summary = pickText(task?.report?.summary, event.summary, event.summary_chars, inBoard);
  if (summary === null) return null;
  return { summary, note: pickText(task?.note, event.note, event.note_chars, inBoard) };
}
