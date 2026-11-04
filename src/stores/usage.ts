// Utilizzo dei coding agent: limiti per coppia (agente, ambiente) e token per
// sessione. Le risposte uguali per valore non cambiano lo stato (il polling
// ripete la stessa lettura per minuti).

import { createStore } from "../lib/store.ts";
import type { SessionUsage, UsageLimitsReply } from "../lib/tauri.ts";

export interface UsageState {
  /** Indicizzati per `UsageTarget.key`. */
  limits: Readonly<Record<string, UsageLimitsReply>>;
  /** Indicizzati per chiave del tab. */
  sessions: Readonly<Record<string, SessionUsage>>;
}

export const INITIAL_USAGE: UsageState = { limits: {}, sessions: {} };

/** Confronto per valore di risposte piccole (evita notifiche con dati identici). */
const sameJson = (a: unknown, b: unknown): boolean => JSON.stringify(a) === JSON.stringify(b);

const has = (record: object, key: string): boolean => Object.prototype.hasOwnProperty.call(record, key);

export function setLimit(s: UsageState, key: string, reply: UsageLimitsReply): UsageState {
  if (has(s.limits, key) && sameJson(s.limits[key], reply)) return s;
  return { ...s, limits: { ...s.limits, [key]: reply } };
}

/** Token di una sessione; `null` = nessun dato (la voce sparisce). */
export function setSession(s: UsageState, key: string, usage: SessionUsage | null): UsageState {
  if (usage === null) return clearSession(s, key);
  if (has(s.sessions, key) && sameJson(s.sessions[key], usage)) return s;
  return { ...s, sessions: { ...s.sessions, [key]: usage } };
}

function clearSession(s: UsageState, key: string): UsageState {
  if (!has(s.sessions, key)) return s;
  const { [key]: _removed, ...rest } = s.sessions;
  return { ...s, sessions: rest };
}

export function clearLimits(s: UsageState): UsageState {
  return Object.keys(s.limits).length === 0 ? s : { ...s, limits: {} };
}

export function clearSessions(s: UsageState): UsageState {
  return Object.keys(s.sessions).length === 0 ? s : { ...s, sessions: {} };
}

/** Tab chiusi: restano solo i dati delle sessioni ancora vive. */
export function retainSessions(s: UsageState, liveKeys: Iterable<string>): UsageState {
  const live = new Set(liveKeys);
  const keys = Object.keys(s.sessions);
  if (keys.every((k) => live.has(k))) return s;
  return { ...s, sessions: Object.fromEntries(keys.filter((k) => live.has(k)).map((k) => [k, s.sessions[k]])) };
}

export function createUsageStore() {
  const store = createStore<UsageState>(INITIAL_USAGE);
  return {
    ...store,
    setLimit: (key: string, reply: UsageLimitsReply) => store.update((s) => setLimit(s, key, reply)),
    setSession: (key: string, usage: SessionUsage | null) => store.update((s) => setSession(s, key, usage)),
    clearLimits: () => store.update(clearLimits),
    clearSessions: () => store.update(clearSessions),
    retainSessions: (liveKeys: Iterable<string>) => store.update((s) => retainSessions(s, liveKeys)),
  };
}

export const usageStore = createUsageStore();
