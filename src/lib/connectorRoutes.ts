// Regole delle fonti (docs/CONNECTORS.MD §5): cosa prendere (filtro dal
// descrittore) e dove metterlo (canale e thread), chi svegliare. Costruzione del
// form, etichette di destinazione e di risveglio. Funzioni pure, importabili
// dall'app e dai test.

import type { ConnectorConfigField } from "../bindings/ConnectorConfigField.ts";
import type { ConnectorRoute } from "../bindings/ConnectorRoute.ts";
import type { ConnectorRouteInput } from "../bindings/ConnectorRouteInput.ts";
import type { ConnectorRouteThread } from "../bindings/ConnectorRouteThread.ts";
import type { HubSecretary } from "../bindings/HubSecretary.ts";
import type { HubThreadSummary } from "../bindings/HubThreadSummary.ts";
import { LABEL_MAX_CHARS } from "./connectors.ts";
import { initialValues, toPayload, validateValues } from "./connectorForm.ts";
import type { FieldErrors, FormValues } from "./connectorForm.ts";
import { GENERAL_CHANNEL, channelName } from "./hub.ts";
import type { HubChannel } from "./hub.ts";
import { mentionable } from "./hubSecretaries.ts";

export type RouteMode = ConnectorRouteThread["mode"];

export const WAKE_NONE = "none";
export const WAKE_OWNER = "owner";
const WAKE_SECRETARY_PREFIX = "secretary:";

/** Le tre destinazioni di una regola, con il testo per la scelta. */
export const ROUTE_MODES: readonly { mode: RouteMode; label: string; help: string }[] = [
  { mode: "existing", label: "Un thread esistente", help: "Tutti gli elementi finiscono nel thread scelto." },
  {
    mode: "new_per_item",
    label: "Un nuovo thread per elemento",
    help: "Ogni elemento apre un thread, intitolato con il suo titolo.",
  },
  {
    mode: "per_conversation",
    label: "Un thread per conversazione",
    help: "Gli elementi della stessa conversazione esterna vanno nello stesso thread.",
  },
];

/** I valori del form di una regola. */
export interface RouteForm {
  label: string;
  filter: FormValues;
  channel: string;
  mode: RouteMode;
  /** Il thread di destinazione, per la modalità `existing`. */
  threadId: string;
  wake: string;
  enabled: boolean;
}

export function newRouteForm(filterFields: readonly ConnectorConfigField[], channel = GENERAL_CHANNEL): RouteForm {
  return {
    label: "",
    filter: initialValues(filterFields, {}),
    channel,
    mode: "existing",
    threadId: "",
    wake: WAKE_NONE,
    enabled: true,
  };
}

export function routeFormOf(route: ConnectorRoute, filterFields: readonly ConnectorConfigField[]): RouteForm {
  return {
    label: route.label,
    filter: initialValues(filterFields, route.filter),
    channel: route.channel,
    mode: route.thread.mode,
    threadId: route.thread.mode === "existing" ? route.thread.thread_id : "",
    wake: route.wake,
    enabled: route.enabled,
  };
}

function threadOf(form: RouteForm): ConnectorRouteThread {
  return form.mode === "existing" ? { mode: "existing", thread_id: form.threadId } : { mode: form.mode };
}

/** Un'etichetta proposta se l'utente non ne scrive una: il primo testo del filtro, altrimenti il nome del connettore. */
export function suggestRouteLabel(
  filterFields: readonly ConnectorConfigField[],
  filter: FormValues,
  connectorLabel: string,
): string {
  const firstText = filterFields
    .map((f) => filter[f.key])
    .find((v): v is string => typeof v === "string" && v.trim() !== "");
  return Array.from(firstText?.trim() ?? connectorLabel)
    .slice(0, LABEL_MAX_CHARS)
    .join("");
}

export interface RouteProblems {
  filter: FieldErrors;
  /** Manca il thread di destinazione. */
  thread: string | null;
}

export function hasProblems(problems: RouteProblems): boolean {
  return problems.thread !== null || Object.keys(problems.filter).length > 0;
}

/** Il controllo locale minimo di una regola. */
export function validateRouteForm(form: RouteForm, filterFields: readonly ConnectorConfigField[]): RouteProblems {
  return {
    filter: validateValues(filterFields, form.filter),
    thread: form.mode === "existing" && form.threadId === "" ? "Scegli il thread di destinazione." : null,
  };
}

/** L'input per il backend. L'etichetta vuota diventa quella proposta. */
export function buildRouteInput(
  form: RouteForm,
  filterFields: readonly ConnectorConfigField[],
  connectorLabel: string,
): ConnectorRouteInput {
  const label = form.label.trim() || suggestRouteLabel(filterFields, form.filter, connectorLabel);
  return {
    label,
    filter: toPayload(filterFields, form.filter),
    channel: form.channel,
    thread: threadOf(form),
    wake: form.wake,
    enabled: form.enabled,
  };
}

// ---------------------------------------------------------------- risveglio

export interface WakeOption {
  value: string;
  label: string;
}

/**
 * Chi si può svegliare alla consegna: nessuno, il titolare del thread e i segretari
 * ammessi nel canale. Un connettore che non può svegliare offre solo «nessuno».
 */
export function wakeOptions(
  secretaries: readonly HubSecretary[],
  channel: string,
  canWake: boolean,
  current: string = WAKE_NONE,
): WakeOption[] {
  const options: WakeOption[] = [{ value: WAKE_NONE, label: "Nessuno (solo il messaggio nel thread)" }];
  if (!canWake) return options;
  options.push({ value: WAKE_OWNER, label: "Il titolare del thread" });
  for (const s of mentionable(secretaries, channel)) {
    options.push({ value: `${WAKE_SECRETARY_PREFIX}${s.name}`, label: `@${s.name}` });
  }
  // Un valore salvato che non è più fra le scelte (segretario tolto dal canale) resta visibile.
  if (!options.some((o) => o.value === current)) options.push({ value: current, label: wakeLabel(current) });
  return options;
}

export function wakeLabel(wake: string): string {
  if (wake === WAKE_NONE) return "Nessun risveglio";
  if (wake === WAKE_OWNER) return "Sveglia il titolare";
  if (wake.startsWith(WAKE_SECRETARY_PREFIX)) return `Sveglia @${wake.slice(WAKE_SECRETARY_PREFIX.length)}`;
  return wake;
}

/** Se il canale cambia, un segretario non più ammesso lì torna a «nessuno». */
export function wakeForChannel(
  wake: string,
  secretaries: readonly HubSecretary[],
  channel: string,
  canWake: boolean,
): string {
  return wakeOptions(secretaries, channel, canWake, WAKE_NONE).some((o) => o.value === wake) ? wake : WAKE_NONE;
}

// ---------------------------------------------------------------- elenchi ed etichette

export function routesOfInstance(routes: readonly ConnectorRoute[], instance: string): ConnectorRoute[] {
  return routes.filter((r) => r.instance === instance);
}

/** Le regole che portano in quel thread (destinazione `existing`). */
export function routesToThread(routes: readonly ConnectorRoute[], channel: string, threadId: string): ConnectorRoute[] {
  return routes.filter((r) => r.channel === channel && r.thread.mode === "existing" && r.thread.thread_id === threadId);
}

/** Il titolo del thread, se l'elenco del canale è stato letto e lo contiene. */
export function threadTitle(
  threads: Readonly<Record<string, readonly HubThreadSummary[]>>,
  channel: string,
  threadId: string,
): string | null {
  return threads[channel]?.find((t) => t.id === threadId)?.title ?? null;
}

/** «Workspace · Titolo del thread», «Workspace · un nuovo thread per elemento», «Workspace · un thread per conversazione». */
export function destinationText(
  route: ConnectorRoute,
  channels: readonly HubChannel[],
  threads: Readonly<Record<string, readonly HubThreadSummary[]>>,
): string {
  const channelLabel = channelName(channels, route.channel);
  const where = route.thread;
  if (where.mode === "new_per_item") return `${channelLabel} · un nuovo thread per elemento`;
  if (where.mode === "per_conversation") return `${channelLabel} · un thread per conversazione`;
  const missing = threads[route.channel] === undefined ? "thread" : "thread non più disponibile";
  return `${channelLabel} · ${threadTitle(threads, route.channel, where.thread_id) ?? missing}`;
}

/** L'input che rifà la regola com'è, con l'abilitazione cambiata (l'interruttore della riga). */
export function routeInputWithEnabled(route: ConnectorRoute, enabled: boolean): ConnectorRouteInput {
  return {
    label: route.label,
    filter: route.filter,
    channel: route.channel,
    thread: route.thread,
    wake: route.wake,
    enabled,
  };
}

/** I canali dei thread a cui puntano le regole (destinazione `existing`), una volta sola ciascuno. */
export function existingTargetChannels(routes: readonly ConnectorRoute[]): string[] {
  return [...new Set(routes.filter((r) => r.thread.mode === "existing").map((r) => r.channel))];
}
