// Connettori dell'Agents HUB (docs/CONNECTORS.MD): raggruppamento dei descrittori,
// etichette e stato di salute delle istanze. Funzioni pure a zero dipendenze,
// importabili sia dall'app sia dai test `node --test`. Nessun connettore è
// nominato qui: tutto viene dai descrittori.

import type { ConnectorDescriptor } from "../bindings/ConnectorDescriptor.ts";
import type { ConnectorHealthState } from "../bindings/ConnectorHealthState.ts";
import type { ConnectorInstanceView } from "../bindings/ConnectorInstanceView.ts";
import type { ConnectorKind } from "../bindings/ConnectorKind.ts";
import type { ConnectorStatus } from "../bindings/ConnectorStatus.ts";

/** Lunghezza massima dell'etichetta di un'istanza o di una regola (come il backend, `limits::LABEL_MAX`). */
export const LABEL_MAX_CHARS = 60;

/** Istanze al massimo per l'intera app (come il backend, `limits::MAX_INSTANCES`). */
export const MAX_INSTANCES = 20;

const MS_PER_SECOND = 1000;
const SECONDS_PER_MINUTE = 60;
const MINUTES_PER_HOUR = 60;

export interface KindSection {
  kind: ConnectorKind;
  title: string;
  description: string;
  /** Il testo dello stato vuoto: nessun connettore di questo tipo è compilato nell'app. */
  empty: string;
}

/** Le due sezioni della pagina, nell'ordine in cui compaiono. */
export const KIND_SECTIONS: readonly KindSection[] = [
  {
    kind: "source",
    title: "Fonti",
    description: "Fanno entrare dati esterni in un thread, secondo regole che scegli tu. Solo in ingresso.",
    empty: "Nessuna fonte disponibile in questa versione.",
  },
  {
    kind: "contact",
    title: "Canali di contatto",
    description: "Portano le notifiche dei thread fuori dall’app e ti permettono di scrivere ai segretari da lì.",
    empty: "Nessun canale di contatto disponibile in questa versione.",
  },
];

/** I descrittori di un tipo, nell'ordine ricevuto. */
export function descriptorsOfKind(
  descriptors: readonly ConnectorDescriptor[],
  kind: ConnectorKind,
): ConnectorDescriptor[] {
  return descriptors.filter((d) => d.kind === kind);
}

export function findDescriptor(
  descriptors: readonly ConnectorDescriptor[],
  connector: string,
): ConnectorDescriptor | undefined {
  return descriptors.find((d) => d.id === connector);
}

/** Le istanze di un connettore, nell'ordine del file. */
export function instancesOfConnector(
  instances: readonly ConnectorInstanceView[],
  connector: string,
): ConnectorInstanceView[] {
  return instances.filter((i) => i.instance.connector === connector);
}

/** Le istanze di un tipo di connettore (le fonti, per le regole), con il loro descrittore. */
export function instancesOfKind(
  instances: readonly ConnectorInstanceView[],
  descriptors: readonly ConnectorDescriptor[],
  kind: ConnectorKind,
): ConnectorInstanceView[] {
  const ids = new Set(descriptorsOfKind(descriptors, kind).map((d) => d.id));
  return instances.filter((i) => ids.has(i.instance.connector));
}

/** Un'altra istanza si può ancora creare. */
export function canCreateInstance(instances: readonly ConnectorInstanceView[]): boolean {
  return instances.length < MAX_INSTANCES;
}

/** Il campo `secret` è impostato (lo slot ha lo stesso nome della chiave del campo). */
export function isSecretSet(view: ConnectorInstanceView, slot: string): boolean {
  return view.secrets.some((s) => s.slot === slot && s.set);
}

// ---------------------------------------------------------------- stato di salute

export type StatusTone = "ok" | "warn" | "error" | "muted";

const STATE_LABELS: Record<ConnectorHealthState, string> = {
  disabled: "Disabilitato",
  starting: "In avvio",
  ok: "Attivo",
  backoff: "Nuovo tentativo",
  needs_auth: "Serve una nuova autorizzazione",
  error: "Errore",
  unavailable: "Non disponibile",
};

const STATE_TONES: Record<ConnectorHealthState, StatusTone> = {
  disabled: "muted",
  starting: "muted",
  ok: "ok",
  backoff: "warn",
  needs_auth: "warn",
  error: "error",
  unavailable: "error",
};

export function stateLabel(state: ConnectorHealthState): string {
  return STATE_LABELS[state];
}

export function stateTone(state: ConnectorHealthState): StatusTone {
  return STATE_TONES[state];
}

/** «tra 40 s», «tra 3 min», «tra 2 h»; «a breve» se l'istante è già passato. */
export function retryIn(nextRetryAt: number, now: number): string {
  const seconds = Math.ceil((nextRetryAt - now) / MS_PER_SECOND);
  if (seconds <= 0) return "a breve";
  if (seconds < SECONDS_PER_MINUTE) return `tra ${seconds} s`;
  const minutes = Math.ceil(seconds / SECONDS_PER_MINUTE);
  if (minutes < MINUTES_PER_HOUR) return `tra ${minutes} min`;
  return `tra ${Math.ceil(minutes / MINUTES_PER_HOUR)} h`;
}

export interface StatusView {
  /** Il testo del chip. */
  label: string;
  tone: StatusTone;
  /** Una riga sotto il chip (il messaggio dell'errore, il prossimo tentativo), se c'è. */
  detail: string | null;
}

/**
 * Cosa mostrare di uno stato: `backoff` dice quando si riprova, `error` e
 * `needs_auth` il messaggio del connettore (già generico, mai risposte del servizio).
 */
export function statusView(status: ConnectorStatus, now: number): StatusView {
  const base = { label: stateLabel(status.state), tone: stateTone(status.state) };
  if (status.state === "backoff") {
    const when = status.next_retry_at === null ? null : `Riprova ${retryIn(status.next_retry_at, now)}`;
    return {
      ...base,
      detail: [status.last_error, when].filter((part) => part !== null && part !== "").join(" · ") || null,
    };
  }
  if (status.state === "error" || status.state === "needs_auth" || status.state === "unavailable") {
    return { ...base, detail: status.last_error };
  }
  return { ...base, detail: null };
}

/** L'interruttore si può usare: un connettore non compilato in questa versione si può solo spegnere. */
export function canToggle(enabled: boolean, status: ConnectorStatus): boolean {
  return enabled || status.state !== "unavailable";
}

/** L'ultimo giro riuscito come ora locale breve, o `null`. */
export function lastOkText(status: ConnectorStatus, locale?: string): string | null {
  if (status.last_ok_at === null) return null;
  return new Date(status.last_ok_at).toLocaleString(locale, { dateStyle: "short", timeStyle: "short" });
}

// ---------------------------------------------------------------- azioni

/** Il testo sotto un codice di abbinamento. */
export const PAIRING_CODE_NOTE = "Vale 10 minuti: mandalo dall’account da abbinare.";

/** Le azioni del descrittore da mostrare come pulsanti: `test` ha il suo («Prova connessione»). */
export function runnableActions(descriptor: ConnectorDescriptor): ConnectorDescriptor["actions"] {
  return descriptor.actions.filter((a) => a.id !== "test");
}

/** Le istanze il cui connettore non è compilato in questa versione (restano salvate, si possono solo eliminare). */
export function orphanInstances(
  instances: readonly ConnectorInstanceView[],
  descriptors: readonly ConnectorDescriptor[],
): ConnectorInstanceView[] {
  const known = new Set(descriptors.map((d) => d.id));
  return instances.filter((i) => !known.has(i.instance.connector));
}
