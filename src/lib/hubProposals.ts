// Proposte nel thread dell'Agents HUB (docs/HUB.MD › "Pianificazione dal thread"):
// stato di una proposta ricavato dagli eventi, task proposti letti dalla board
// e testi della scheda. Funzioni pure a zero dipendenze, importabili sia
// dall'app sia dai test `node --test`.

import type { BoardView } from "../bindings/BoardView.ts";
import type { HubEvent } from "../bindings/HubEvent.ts";
import type { HubMessage } from "../bindings/HubMessage.ts";
import type { TaskSpec } from "../bindings/TaskSpec.ts";
import type { TaskView } from "../bindings/TaskView.ts";
import { isPlan } from "./plan.ts";

export type ProposalStatus = "pending" | "accepted" | "rejected";

export const PROPOSAL_STATUS_LABEL: Readonly<Record<ProposalStatus, string>> = {
  pending: "In attesa",
  accepted: "Accettata",
  rejected: "Scartata",
};

export const PROPOSAL_REPLY_HINT = "Rispondi per accettare o scartare";

const STATUS_OF_EVENT: Readonly<Record<string, ProposalStatus>> = {
  proposal_created: "pending",
  proposal_accepted: "accepted",
  proposal_rejected: "rejected",
};

/** Il numero di task nel testo a template di una nuova proposta (`events/texts.rs`: «Nuova proposta: N task»). */
const COUNT_IN_BODY = /^Nuova proposta: (\d+) task/;

/** Stato di ogni proposta del thread: vale l'ultimo evento che la riguarda. */
export function proposalStatuses(messages: readonly Pick<HubMessage, "event">[]): Record<string, ProposalStatus> {
  const statuses: Record<string, ProposalStatus> = {};
  for (const { event } of messages) {
    const status = event?.proposal_id ? STATUS_OF_EVENT[event.kind] : undefined;
    if (status && event?.proposal_id) statuses[event.proposal_id] = status;
  }
  return statuses;
}

/** I task della proposta, se è ancora pendente nella board del workspace; `null` altrimenti (decisa, o board non letta). */
export function proposalSpecs(
  boards: Readonly<Record<string, Pick<BoardView, "proposals">>>,
  workspace: string | null,
  proposalId: string,
): readonly TaskSpec[] | null {
  if (workspace === null) return null;
  return boards[workspace]?.proposals.find((p) => p.id === proposalId)?.tasks ?? null;
}

/** Quanti task propone: dall'elenco letto dalla board, altrimenti dal testo del messaggio; `null` se non si sa. */
export function proposalCount(specs: readonly TaskSpec[] | null, body: string): number | null {
  if (specs !== null) return specs.length;
  const match = COUNT_IN_BODY.exec(body);
  return match ? Number(match[1]) : null;
}

/** «Proposta pronta · 3 task», «Proposta pronta · 1 task»; senza numero, solo «Proposta pronta». */
export function proposalTitle(count: number | null): string {
  return count === null ? "Proposta pronta" : `Proposta pronta · ${count} task`;
}

/** Il task a cui si riferisce l'evento, se è ancora nella board del suo workspace (eliminato o archiviato: `null`). */
export function eventTaskOf(
  boards: Readonly<Record<string, Pick<BoardView, "tasks">>>,
  event: Pick<HubEvent, "workspace" | "task_id">,
): TaskView | null {
  if (event.workspace === null || event.task_id === null) return null;
  return boards[event.workspace]?.tasks.find((t) => t.id === event.task_id) ?? null;
}

/** Testo del link che apre il task dell'evento nel Tasks Manager: i piani hanno il loro. */
export function taskLinkLabel(task: Pick<TaskView, "kind">): string {
  return isPlan(task) ? "apri il task di pianificazione nel Tasks Manager" : "apri il task nel Tasks Manager";
}

/** Stato da mostrare sulla scheda di un messaggio: solo «proposta pronta» è una scheda, gli altri eventi restano righe. */
export function cardStatus(
  message: Pick<HubMessage, "event">,
  statuses: Readonly<Record<string, ProposalStatus>>,
): ProposalStatus | undefined {
  const event = message.event;
  return event?.kind === "proposal_created" && event.proposal_id ? statuses[event.proposal_id] : undefined;
}
