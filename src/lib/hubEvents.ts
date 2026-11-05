// Resa dei messaggi evento dell'Agents HUB: etichetta, icona e tono per tipo.
// Il backend può aggiungere tipi (compatibile in lettura): uno sconosciuto si
// mostra con la resa generica, mai come errore.

import type { BoardView } from "../bindings/BoardView.ts";
import type { HubEvent } from "../bindings/HubEvent.ts";

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
