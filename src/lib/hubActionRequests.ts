// Richieste di azione dei segretari nel thread dell'Agents HUB (docs/HUB.MD ›
// "Azioni che chiedono conferma"): in che stato mostrare la scheda, con quali
// etichette. Funzioni pure a zero dipendenze, importabili sia dall'app sia dai
// test `node --test`.

import type { HubActionRequest } from "../bindings/HubActionRequest.ts";
import type { HubActionRequestState } from "../bindings/HubActionRequestState.ts";
import type { HubMessage } from "../bindings/HubMessage.ts";
import type { HubFieldChange } from "../bindings/HubFieldChange.ts";
import type { HubRequestedAction } from "../bindings/HubRequestedAction.ts";
import type { HubTaskField } from "../bindings/HubTaskField.ts";
import { ACTION_LABELS } from "./board.ts";

/** Stato da mostrare: quello del backend, ma una richiesta in attesa oltre la scadenza è scaduta. */
export type DisplayState = HubActionRequestState;

export const REQUEST_CONFIRM_LABEL = "Conferma";
export const REQUEST_CANCEL_LABEL = "Annulla";
export const REQUEST_DEPENDENTS_LABEL = "Verranno eliminati anche";
export const REQUEST_UPDATE_LABEL = "Modifica";
export const REQUEST_CHANGES_LABEL = "Modifiche";

const STATE_LABELS: Readonly<Record<HubActionRequestState, string>> = {
  pending: "In attesa",
  confirmed: "Confermata",
  cancelled: "Annullata",
  expired: "Scaduta",
  stale: "Non più valida",
  failed: "Non eseguita",
};

/** Lo stato da mostrare adesso: una richiesta in attesa con `now >= expires_at` è scaduta, senza pulsanti. */
export function displayState(request: Pick<HubActionRequest, "state" | "expires_at">, now: number): DisplayState {
  return request.state === "pending" && now >= request.expires_at ? "expired" : request.state;
}

/** Etichetta dello stato mostrato. */
export function requestStateLabel(state: DisplayState): string {
  return STATE_LABELS[state];
}

/** Etichetta dell'azione chiesta, la stessa del Tasks Manager. */
export function requestActionLabel(action: HubRequestedAction): string {
  return action === "update" ? REQUEST_UPDATE_LABEL : ACTION_LABELS[action];
}

const FIELD_LABELS: Readonly<Record<HubTaskField, string>> = {
  title: "Titolo",
  task: "Specifica",
  depends_on: "Dipendenze",
  model: "Modello",
  bot: "Bot",
};

/** Cosa si legge quando un campo non ha valore. */
const EMPTY_VALUES: Readonly<Record<HubTaskField, string>> = {
  title: "—",
  task: "—",
  depends_on: "nessuna",
  model: "predefinito",
  bot: "nessuno",
};

/** Etichetta di un campo del task. */
export function fieldLabel(field: HubTaskField): string {
  return FIELD_LABELS[field];
}

/** Il valore di prima o di dopo di un campo, con una parola al posto del vuoto. */
export function changeValue(change: Pick<HubFieldChange, "field">, value: string | null): string {
  return value ?? EMPTY_VALUES[change.field];
}

/** Titolo della scheda: «@ugo chiede: Elimina». */
export function requestHeading(request: Pick<HubActionRequest, "secretary" | "action">): string {
  return `@${request.secretary} chiede: ${requestActionLabel(request.action)}`;
}

/** I pulsanti «Conferma» e «Annulla» si mostrano solo finché la richiesta aspetta l'utente. */
export function canDecide(request: Pick<HubActionRequest, "state" | "expires_at">, now: number): boolean {
  return displayState(request, now) === "pending";
}

/** La richiesta di un messaggio «richiesta di azione» (scheda), altrimenti `null`. */
export function requestOf(message: Pick<HubMessage, "event">): HubActionRequest | null {
  const event = message.event;
  return event?.kind === "action_request" ? event.request : null;
}
