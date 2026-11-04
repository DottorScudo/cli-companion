// Stato volatile dei tab (titolo OSC, agente, automazione, busy), per chiave.
// Ogni transizione ritorna lo stesso stato se non cambia nulla, così l'archivio
// non notifica. Chiave sconosciuta (tab chiuso nel frattempo): nessun effetto.

import { createStore } from "../lib/store.ts";
import { applyAgentEvent, applyUserInput, tabStatus } from "../lib/agents.ts";
import type { AgentEventLike, AgentTabState, TabStatus } from "../lib/agents.ts";
import { applyAutomationUpdate } from "../lib/automation.ts";
import type { AutomationMeta, AutomationUpdateLike } from "../lib/automation.ts";

export interface TabRuntime {
  /** Titolo OSC. */
  title?: string;
  /** Stato dell'agente (assente = tab normale con euristica busy/idle). */
  agent?: AgentTabState;
  /** Metadati del worker (assente = tab normale). */
  automation?: AutomationMeta;
  /** Euristica della shell: sta producendo output. */
  busy: boolean;
}

export type RuntimeState = Readonly<Record<string, TabRuntime>>;

const has = (s: RuntimeState, key: string): boolean => Object.prototype.hasOwnProperty.call(s, key);

/** Applica `fn` alla voce di `key`; voce assente o risultato identico: stesso stato. */
function patch(s: RuntimeState, key: string, fn: (rt: TabRuntime) => TabRuntime): RuntimeState {
  if (!has(s, key)) return s;
  const next = fn(s[key]);
  return next === s[key] ? s : { ...s, [key]: next };
}

/** Voce iniziale di un tab, senza i campi `undefined`. Esistente: invariata. */
export function seed(s: RuntimeState, key: string, init: Partial<TabRuntime> = {}): RuntimeState {
  if (has(s, key)) return s;
  const defined = Object.fromEntries(Object.entries(init).filter(([, v]) => v !== undefined));
  return { ...s, [key]: { busy: false, ...defined } };
}

export function setTitle(s: RuntimeState, key: string, title: string): RuntimeState {
  return patch(s, key, (rt) => (rt.title === title ? rt : { ...rt, title }));
}

export function agentEvent(s: RuntimeState, key: string, ev: AgentEventLike): RuntimeState {
  return patch(s, key, (rt) => {
    const agent = applyAgentEvent(rt.agent, ev);
    return agent === rt.agent ? rt : { ...rt, agent };
  });
}

/** Input dell'utente: Invio => l'agente lavora, altro input toglie ✓/❗. */
export function userInput(s: RuntimeState, key: string, enter: boolean): RuntimeState {
  return patch(s, key, (rt) => {
    const agent = applyUserInput(rt.agent, enter);
    return agent === rt.agent ? rt : { ...rt, agent };
  });
}

export function setBusy(s: RuntimeState, key: string, busy: boolean): RuntimeState {
  return patch(s, key, (rt) => (rt.busy === busy ? rt : { ...rt, busy }));
}

/** Aggiorna i metadati del worker; un tab non automatico non ne acquista. */
export function automationUpdate(s: RuntimeState, key: string, u: AutomationUpdateLike): RuntimeState {
  return patch(s, key, (rt) => {
    if (!rt.automation) return rt;
    const automation = applyAutomationUpdate(rt.automation, u);
    return automation === rt.automation ? rt : { ...rt, automation };
  });
}

export function remove(s: RuntimeState, key: string): RuntimeState {
  if (!has(s, key)) return s;
  const { [key]: _removed, ...rest } = s;
  return rest;
}

/** Cosa mostrare per un tab: per i tab-agente lo stato sostituisce l'euristica busy/idle. */
export function statusOf(rt: TabRuntime | undefined): TabStatus {
  return tabStatus(rt?.agent, rt?.busy === true);
}

/** Stesso stato mostrato? (`tabStatus` crea un oggetto nuovo a ogni chiamata) */
export function sameStatus(a: TabStatus | null, b: TabStatus | null): boolean {
  if (a === null || b === null) return a === b;
  return a.icon === b.icon && a.tone === b.tone && a.label === b.label;
}

export function createRuntimeStore() {
  const store = createStore<RuntimeState>({});
  return {
    ...store,
    seed: (key: string, init?: Partial<TabRuntime>) => store.update((s) => seed(s, key, init)),
    setTitle: (key: string, title: string) => store.update((s) => setTitle(s, key, title)),
    agentEvent: (key: string, ev: AgentEventLike) => store.update((s) => agentEvent(s, key, ev)),
    userInput: (key: string, enter: boolean) => store.update((s) => userInput(s, key, enter)),
    setBusy: (key: string, busy: boolean) => store.update((s) => setBusy(s, key, busy)),
    automationUpdate: (key: string, u: AutomationUpdateLike) => store.update((s) => automationUpdate(s, key, u)),
    remove: (key: string) => store.update((s) => remove(s, key)),
    get: (key: string): TabRuntime | undefined => (has(store.getState(), key) ? store.getState()[key] : undefined),
  };
}

export const runtimeStore = createRuntimeStore();
