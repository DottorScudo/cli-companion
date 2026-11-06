// Editor dei Bot-agente (Impostazioni › Bot): dal Bot al modulo, dal modulo alla
// bozza per il backend, validazione e riepiloghi. Funzioni pure a zero
// dipendenze (solo i tipi dei bindings), importabili sia dall'app sia dai test.
// Nessun nome di agente o di modello: tutto viene dai dati delle integrazioni.

import type { AgentBot } from "../bindings/AgentBot.ts";
import type { AgentBotDraft } from "../bindings/AgentBotDraft.ts";
import type { BotImage } from "../bindings/BotImage.ts";
import type { BotSkillOption } from "../bindings/BotSkillOption.ts";
import type { Effort } from "../bindings/Effort.ts";
import type { IntegrationInfo } from "../bindings/IntegrationInfo.ts";
import type { ModelChoice } from "../bindings/ModelChoice.ts";
import type { SkillRef } from "../bindings/SkillRef.ts";
import { BOT_NAME_MAX } from "./bots.ts";
import { modelError } from "./model.ts";

/** Id del Bot che c'è sempre: non si elimina né si disattiva. */
export const GENERAL_BOT_ID = "general";
/** Lunghezza massima del compito (come `TASK_MAX_CHARS` di `validate.rs`). */
export const BOT_TASK_MAX = 300;

/** Valore del selettore dell'agente per «segue il workspace». */
export const FOLLOW_WORKSPACE = "";
export const FOLLOW_WORKSPACE_LABEL = "Segue il workspace";
export const AGENT_UNAVAILABLE_SUFFIX = "(non disponibile)";
export const EFFORT_IGNORED_NOTE =
  "Con «Segue il workspace» l’effort vale solo se l’agente del workspace lo supporta: sugli altri viene ignorato.";

/** Contratti che un agente deve dichiarare per fare da Bot (worker e avvio interattivo). */
const BOT_CAPABILITIES = ["automation", "interactive-launch"] as const;

/** Livelli di effort nell'ordine in cui si mostrano, con l'etichetta in italiano. */
export const EFFORT_LABELS: Readonly<Record<Effort, string>> = {
  low: "Bassa",
  medium: "Media",
  high: "Alta",
  xhigh: "Molto alta",
  max: "Massima",
};
const EFFORT_ORDER = Object.keys(EFFORT_LABELS) as Effort[];

export type ModelKind = "default" | "standard" | "strong" | "id";

/** Voci del selettore del modello; `id` (modello concreto) richiede l'agente fissato. */
export const MODEL_KIND_LABELS: Readonly<Record<ModelKind, string>> = {
  default: "Predefinito dell’agente",
  standard: "Standard",
  strong: "Forte",
  id: "Modello preciso…",
};

/** Il modulo dell'editor: stringhe e interruttori, come li tengono i controlli. */
export interface BotForm {
  name: string;
  task: string;
  /** Id dell'agente, `FOLLOW_WORKSPACE` se segue il workspace. */
  agent: string;
  modelKind: ModelKind;
  /** Modello concreto, usato solo con `modelKind === "id"`. */
  modelId: string;
  /** `""` = nessun effort. */
  effort: Effort | "";
  /** `""` = nessuna skill, altrimenti `skillValue`. */
  skill: string;
  enabled: boolean;
  image: BotImage | null;
}

export type BotField = "name" | "task" | "agent" | "model" | "effort" | "skill" | "image";
export type BotFormErrors = Partial<Record<BotField, string>>;

export function emptyBotForm(): BotForm {
  return {
    name: "",
    task: "",
    agent: FOLLOW_WORKSPACE,
    modelKind: "default",
    modelId: "",
    effort: "",
    skill: "",
    enabled: true,
    image: null,
  };
}

export const isGeneralBot = (bot: Pick<AgentBot, "id">): boolean => bot.id === GENERAL_BOT_ID;

// ---------------------------------------------------------------------------
// Skill
// ---------------------------------------------------------------------------

/** Valore di un'opzione del selettore delle skill: `plugin:<id>` o `user:<id>`. */
export function skillValue(skill: SkillRef | null): string {
  return skill === null ? "" : `${skill.kind}:${skill.id}`;
}

/** Inverso di `skillValue`; `null` per «nessuna skill» o un valore non riconosciuto. */
export function skillFromValue(value: string): SkillRef | null {
  const at = value.indexOf(":");
  if (at < 0) return null;
  const kind = value.slice(0, at);
  const id = value.slice(at + 1);
  return (kind === "plugin" || kind === "user") && id !== "" ? { kind, id } : null;
}

// ---------------------------------------------------------------------------
// Dal Bot al modulo e ritorno
// ---------------------------------------------------------------------------

function modelParts(model: ModelChoice): Pick<BotForm, "modelKind" | "modelId"> {
  switch (model.kind) {
    case "default":
      return { modelKind: "default", modelId: "" };
    case "tier":
      return { modelKind: model.value, modelId: "" };
    case "id":
      return { modelKind: "id", modelId: model.value };
  }
}

export function formFromBot(bot: AgentBot): BotForm {
  return {
    name: bot.name,
    task: bot.task,
    agent: bot.agent ?? FOLLOW_WORKSPACE,
    ...modelParts(bot.model),
    effort: bot.effort ?? "",
    skill: skillValue(bot.skill),
    enabled: bot.enabled,
    image: bot.image,
  };
}

function modelChoice(form: Pick<BotForm, "modelKind" | "modelId">): ModelChoice {
  switch (form.modelKind) {
    case "default":
      return { kind: "default" };
    case "id":
      return { kind: "id", value: form.modelId.trim() };
    default:
      return { kind: "tier", value: form.modelKind };
  }
}

/** La bozza per il backend, che la convalida di nuovo. */
export function draftFromForm(form: BotForm): AgentBotDraft {
  return {
    name: form.name.trim(),
    task: form.task.trim(),
    agent: form.agent === FOLLOW_WORKSPACE ? null : form.agent,
    model: modelChoice(form),
    effort: form.effort === "" ? null : form.effort,
    skill: skillFromValue(form.skill),
    enabled: form.enabled,
    image: form.image,
  };
}

export const draftFromBot = (bot: AgentBot): AgentBotDraft => draftFromForm(formFromBot(bot));

/** Il modulo è diverso dal Bot salvato (o da un modulo vuoto, per un Bot nuovo). */
export function isFormDirty(form: BotForm, saved: BotForm): boolean {
  return (Object.keys(form) as (keyof BotForm)[]).some((key) =>
    key === "image" ? form.image?.png !== saved.image?.png : form[key] !== saved[key],
  );
}

// ---------------------------------------------------------------------------
// Agenti, modelli ed effort
// ---------------------------------------------------------------------------

export interface AgentChoice {
  id: string;
  label: string;
}

/** Agenti fissabili: abilitati e con i contratti dei Bot. */
export function botAgents(integrations: readonly IntegrationInfo[]): IntegrationInfo[] {
  return integrations.filter((i) => i.enabled && BOT_CAPABILITIES.every((c) => i.capabilities.includes(c)));
}

/**
 * Voci del selettore dell'agente: «Segue il workspace», gli agenti fissabili e,
 * se `selected` non è fra questi (agente poi disabilitato), la voce corrente in coda.
 */
export function agentChoices(integrations: readonly IntegrationInfo[], selected: string): AgentChoice[] {
  const choices: AgentChoice[] = [
    { id: FOLLOW_WORKSPACE, label: FOLLOW_WORKSPACE_LABEL },
    ...botAgents(integrations).map((i) => ({ id: i.id, label: i.name })),
  ];
  if (!choices.some((c) => c.id === selected)) {
    const name = integrations.find((i) => i.id === selected)?.name ?? selected;
    choices.push({ id: selected, label: `${name} ${AGENT_UNAVAILABLE_SUFFIX}` });
  }
  return choices;
}

const agentOf = (integrations: readonly IntegrationInfo[], id: string): IntegrationInfo | undefined =>
  integrations.find((i) => i.id === id);

/** Modelli consigliati dall'agente fissato (vuoti se non è fissato o non li dichiara). */
export function modelSuggestionsOf(integrations: readonly IntegrationInfo[], agent: string) {
  return agentOf(integrations, agent)?.models ?? [];
}

export type EffortChoices =
  /** L'agente scelto non supporta l'effort: il selettore non si mostra. */
  { show: false } | { show: true; levels: Effort[]; note: string | null };

/**
 * Livelli di effort scelti per l'agente. Fissato: i suoi. Segue il workspace:
 * quelli di un qualunque agente fissabile, con la nota che sugli altri vale
 * come ignorato.
 */
export function effortChoices(integrations: readonly IntegrationInfo[], agent: string): EffortChoices {
  const supported =
    agent === FOLLOW_WORKSPACE
      ? new Set(botAgents(integrations).flatMap((i) => i.effort_levels))
      : new Set(agentOf(integrations, agent)?.effort_levels ?? []);
  const levels = EFFORT_ORDER.filter((level) => supported.has(level));
  if (levels.length === 0) return { show: false };
  return { show: true, levels, note: agent === FOLLOW_WORKSPACE ? EFFORT_IGNORED_NOTE : null };
}

/**
 * Modulo dopo il cambio di agente: il modello preciso resta solo se il nuovo
 * agente lo consiglia (i nomi sono dell'agente) e l'effort solo se lo supporta.
 */
export function formAfterAgentChange(integrations: readonly IntegrationInfo[], form: BotForm, agent: string): BotForm {
  const next: BotForm = { ...form, agent };
  if (next.modelKind === "id" && !modelSuggestionsOf(integrations, agent).some((m) => m.id === next.modelId.trim())) {
    next.modelKind = "default";
    next.modelId = "";
  }
  const efforts = effortChoices(integrations, agent);
  if (next.effort !== "" && !(efforts.show && efforts.levels.includes(next.effort))) next.effort = "";
  return next;
}

// ---------------------------------------------------------------------------
// Validazione
// ---------------------------------------------------------------------------

const CONTROL_CHARS = /[\u0000-\u001f\u007f-\u009f]/;
const lengthOf = (text: string): number => Array.from(text).length;

function nameError(
  raw: string,
  taken: { bots: readonly string[]; secretaries: readonly string[] },
): string | undefined {
  const name = raw.trim();
  if (name === "" || lengthOf(name) > BOT_NAME_MAX) return `Il nome deve avere da 1 a ${BOT_NAME_MAX} caratteri`;
  if (CONTROL_CHARS.test(name)) return "Il nome contiene caratteri non validi";
  const key = name.toLowerCase();
  if (taken.bots.some((n) => n.toLowerCase() === key)) return `C’è già un Bot di nome “${name}”`;
  if (taken.secretaries.some((n) => n.toLowerCase() === key)) return `“${name}” è già il nome di un segretario`;
  return undefined;
}

function taskError(raw: string): string | undefined {
  const task = raw.trim();
  if (task === "" || lengthOf(task) > BOT_TASK_MAX) return `Il compito deve avere da 1 a ${BOT_TASK_MAX} caratteri`;
  if (CONTROL_CHARS.test(task)) return "Il compito deve stare su una riga";
  return undefined;
}

function modelFieldError(form: BotForm): string | undefined {
  if (form.modelKind !== "id") return undefined;
  if (form.agent === FOLLOW_WORKSPACE) return "Per scegliere un modello preciso fissa prima l’agente";
  if (form.modelId.trim() === "") return "Scrivi il nome del modello";
  return modelError(form.modelId) ?? undefined;
}

export interface TakenNames {
  /** Nomi degli altri Bot (non quello in modifica). */
  bots: readonly string[];
  /** Nomi dei segretari dell'Agents HUB. */
  secretaries: readonly string[];
}

/** Errori del modulo per campo; vuoto = si può salvare. Il backend convalida comunque. */
export function validateBotForm(form: BotForm, taken: TakenNames): BotFormErrors {
  const errors: BotFormErrors = {
    name: nameError(form.name, taken),
    task: taskError(form.task),
    model: modelFieldError(form),
  };
  return Object.fromEntries(Object.entries(errors).filter(([, message]) => message !== undefined));
}

/** Campo a cui appartiene un errore del backend, dal suo testo; `null` se non si capisce. */
export function fieldOfBackendError(message: string): BotField | null {
  const text = message.toLowerCase();
  const rules: [BotField, RegExp][] = [
    ["name", /\bnome\b|\bbot di nome\b|segretario/],
    ["task", /compito/],
    ["effort", /effort/],
    ["skill", /skill/],
    ["image", /immagine|\bpng\b|colore/],
    ["model", /modell/],
    ["agent", /agente|integrazion|non si può usare/],
  ];
  return rules.find(([, pattern]) => pattern.test(text))?.[0] ?? null;
}

// ---------------------------------------------------------------------------
// Riepilogo nell'elenco
// ---------------------------------------------------------------------------

/** Riepilogo di un Bot: una voce per ogni scelta fatta, nell'ordine agente, modello, effort, skill. */
export function botSummary(
  bot: AgentBot,
  integrations: readonly IntegrationInfo[],
  skills: readonly BotSkillOption[],
): string[] {
  const form = formFromBot(bot);
  const agent = bot.agent === null ? FOLLOW_WORKSPACE_LABEL : (agentOf(integrations, bot.agent)?.name ?? bot.agent);
  const model = form.modelKind === "id" ? form.modelId : MODEL_KIND_LABELS[form.modelKind];
  const skill = skills.find((s) => skillValue(s.skill) === form.skill)?.name ?? bot.skill?.id ?? null;
  return [
    agent,
    model,
    ...(bot.effort === null ? [] : [`Effort ${EFFORT_LABELS[bot.effort].toLowerCase()}`]),
    ...(skill === null ? [] : [skill]),
  ];
}
