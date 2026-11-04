// Persistenza della sessione (tab, gruppi, layout) tra un avvio e l'altro:
// schema, validazione tollerante e comando di ripristino. Funzioni pure a zero
// dipendenze, importabili sia dall'app sia dai test `node --test`
// (type-stripping). Il backend (session.rs) conserva solo un blob opaco.

import { isResumeCommand, profileForCommand } from "./agents.ts";
import type { CommandProfileLike, SessionResumeLike } from "./agents.ts";
import { sanitizeBotName } from "./bots.ts";

export const SESSION_VERSION = 1;

/** Limite del comando salvato (stesso di `LAUNCH_COMMAND_MAX` in launch.ts). */
const COMMAND_MAX = 1000;
const TITLE_MAX = 80;
const TAB_ID_MAX = 64;
/** Scrollback: righe serializzate per terminale. */
export const SCROLLBACK_LINES = 2000;
/** Attesa dall'output prima di salvare lo scrollback. */
export const SCROLLBACK_SAVE_MS = 5000;
/** Attesa dopo una modifica prima di salvare la sessione. */
export const SESSION_SAVE_MS = 500;
/** Attesa massima dalla prima modifica in sospeso, anche se continuano ad arrivarne. */
export const SESSION_SAVE_MAX_MS = 2000;

/**
 * Ritardo del prossimo salvataggio: debounce di `waitMs`, ma mai oltre
 * `maxWaitMs` dalla prima modifica in sospeso (`pendingSince`). Cosi' i
 * cambi frequenti (titoli OSC) non rimandano il salvataggio all'infinito.
 */
export function debounceDelay(now: number, pendingSince: number, waitMs: number, maxWaitMs: number): number {
  return Math.max(0, Math.min(waitMs, pendingSince + maxWaitMs - now));
}
/** Tempo massimo di attesa dei salvataggi alla chiusura dell'app. */
export const CLOSE_FLUSH_TIMEOUT_MS = 3000;

/** Riga di separazione scritta tra la cronologia salvata e la nuova sessione. */
export const RESTORE_SEPARATOR = "— sessione ripristinata —";

export interface SessionTab {
  tabId: string;
  workspaceId: string;
  launchId?: string;
  /** Comando originale dell'avvio ("" = shell semplice). */
  command: string;
  title?: string;
  agentId?: string;
  agentSessionId?: string;
  /** Nome del bot della sessione (vedi bots.ts); assente nelle sessioni vecchie. */
  botName?: string;
}

export interface SessionGroup {
  workspaceId: string;
  order: string[];
  activeTabId: string | null;
  maximizedTabId: string | null;
}

/** Vista globale (tab fissati). Assente nelle sessioni scritte prima della 0.5.1. */
export interface SessionGlobal {
  order: string[];
  activeTabId: string | null;
  maximizedTabId: string | null;
}

export interface SessionData {
  version: number;
  activeWorkspaceId: string | null;
  /** "global" solo se la vista globale ha almeno un tab. */
  activeView: "workspace" | "global";
  groups: SessionGroup[];
  global: SessionGlobal;
  tabs: SessionTab[];
}

/** Id tab ammesso (stesso insieme di `is_valid_tab_id` in agent_notify.rs). */
export function isSafeTabId(id: unknown): id is string {
  return typeof id === "string" && id.length <= TAB_ID_MAX && /^[A-Za-z0-9_-]+$/.test(id);
}

/** Id di sessione dell'agente: finisce in un comando, solo caratteri sicuri. */
export function isSafeAgentSessionId(id: unknown): id is string {
  return typeof id === "string" && id.length <= 100 && /^[A-Za-z0-9_-]+$/.test(id);
}

export function serializeSession(data: Omit<SessionData, "version">): string {
  const full: SessionData = { version: SESSION_VERSION, ...data };
  return JSON.stringify(full);
}

const isObj = (v: unknown): v is Record<string, unknown> => typeof v === "object" && v !== null && !Array.isArray(v);

const optStr = (v: unknown, max: number): string | undefined =>
  typeof v === "string" && v.trim() !== "" ? v.slice(0, max) : undefined;

function parseTab(raw: unknown, workspaceIds: Set<string>): SessionTab | null {
  if (!isObj(raw)) return null;
  const { tabId, workspaceId } = raw;
  if (!isSafeTabId(tabId)) return null;
  if (typeof workspaceId !== "string" || !workspaceIds.has(workspaceId)) return null;
  const tab: SessionTab = {
    tabId,
    workspaceId,
    command: typeof raw.command === "string" ? raw.command.trim().slice(0, COMMAND_MAX) : "",
  };
  const launchId = optStr(raw.launchId, 100);
  if (launchId) tab.launchId = launchId;
  const title = optStr(raw.title, TITLE_MAX);
  if (title) tab.title = title;
  const agentId = optStr(raw.agentId, 64);
  if (agentId) tab.agentId = agentId;
  if (isSafeAgentSessionId(raw.agentSessionId)) tab.agentSessionId = raw.agentSessionId;
  const botName = sanitizeBotName(raw.botName);
  if (botName) tab.botName = botName;
  return tab;
}

/**
 * Legge e valida il blob salvato. Ritorna `null` se non e' JSON, ha una
 * versione sconosciuta o non resta nulla da ripristinare. Tab con id non
 * valido/duplicato o di un workspace che non esiste piu' vengono scartati; i
 * gruppi si ricostruiscono dai tab sopravvissuti (un tab fuori da ogni
 * `order` viene accodato al gruppo del suo workspace).
 */
export function parseSession(raw: string | null | undefined, workspaceIds: Iterable<string>): SessionData | null {
  if (typeof raw !== "string" || raw.trim() === "") return null;
  let doc: unknown;
  try {
    doc = JSON.parse(raw);
  } catch {
    return null;
  }
  if (!isObj(doc) || doc.version !== SESSION_VERSION) return null;
  const known = new Set(workspaceIds);

  const tabs: SessionTab[] = [];
  const byId = new Map<string, SessionTab>();
  for (const t of Array.isArray(doc.tabs) ? doc.tabs : []) {
    const tab = parseTab(t, known);
    if (tab && !byId.has(tab.tabId)) {
      byId.set(tab.tabId, tab);
      tabs.push(tab);
    }
  }
  if (tabs.length === 0) return null;

  const groups: SessionGroup[] = [];
  const placed = new Set<string>();
  const groupOf = (workspaceId: string) => groups.find((g) => g.workspaceId === workspaceId);
  for (const g of Array.isArray(doc.groups) ? doc.groups : []) {
    if (!isObj(g) || typeof g.workspaceId !== "string" || groupOf(g.workspaceId)) continue;
    const order: string[] = [];
    for (const id of Array.isArray(g.order) ? g.order : []) {
      const tab = typeof id === "string" ? byId.get(id) : undefined;
      if (tab && tab.workspaceId === g.workspaceId && !placed.has(tab.tabId)) {
        placed.add(tab.tabId);
        order.push(tab.tabId);
      }
    }
    if (order.length === 0) continue;
    groups.push({
      workspaceId: g.workspaceId,
      order,
      activeTabId: typeof g.activeTabId === "string" && order.includes(g.activeTabId) ? g.activeTabId : order[0],
      maximizedTabId:
        typeof g.maximizedTabId === "string" && order.includes(g.maximizedTabId) ? g.maximizedTabId : null,
    });
  }
  for (const tab of tabs) {
    if (placed.has(tab.tabId)) continue;
    const g = groupOf(tab.workspaceId);
    if (g) g.order.push(tab.tabId);
    else {
      groups.push({
        workspaceId: tab.workspaceId,
        order: [tab.tabId],
        activeTabId: tab.tabId,
        maximizedTabId: null,
      });
    }
  }

  const active =
    typeof doc.activeWorkspaceId === "string" && groups.some((g) => g.workspaceId === doc.activeWorkspaceId)
      ? doc.activeWorkspaceId
      : groups[0].workspaceId;

  // Vista globale (campi opzionali: le sessioni v1 precedenti non li hanno,
  // quindi la versione resta 1). Si tengono solo i tab sopravvissuti.
  const rawGlobal = isObj(doc.global) ? doc.global : {};
  const pinned: string[] = [];
  for (const id of Array.isArray(rawGlobal.order) ? rawGlobal.order : []) {
    if (typeof id === "string" && byId.has(id) && !pinned.includes(id)) pinned.push(id);
  }
  const global: SessionGlobal = {
    order: pinned,
    activeTabId:
      typeof rawGlobal.activeTabId === "string" && pinned.includes(rawGlobal.activeTabId)
        ? rawGlobal.activeTabId
        : (pinned[0] ?? null),
    maximizedTabId:
      typeof rawGlobal.maximizedTabId === "string" && pinned.includes(rawGlobal.maximizedTabId)
        ? rawGlobal.maximizedTabId
        : null,
  };
  const activeView = doc.activeView === "global" && pinned.length > 0 ? "global" : "workspace";
  return { version: SESSION_VERSION, activeWorkspaceId: active, activeView, groups, global, tabs };
}

// ---------------------------------------------------------------------------
// Comando di ripristino
// ---------------------------------------------------------------------------

/** Profilo di un agente come serve a `restoreCommand` (campi di `AgentProfile` in tauri.ts). */
export interface RestoreProfileLike extends CommandProfileLike {
  is_default: boolean;
}

const escapeRegExp = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/**
 * Toglie dal comando i flag di ripresa dichiarati dall'agente (`--flag x`,
 * `--flag=x` per quelli con valore), lasciando intatto il resto (spaziatura
 * e virgolette comprese).
 */
function stripResumeFlags(command: string, flags: SessionResumeLike["strip_flags"]): string {
  let result = command;
  for (const flag of flags) {
    if (flag.names.length === 0) continue;
    const names = flag.names.map(escapeRegExp).join("|");
    const value = flag.takes_value ? "(?:=\\S+|\\s+(?!-)\\S+)?" : "";
    result = result.replace(new RegExp(`(^|\\s)(?:${names})${value}(?=\\s|$)`, "g"), "$1");
  }
  return result.replace(/\s{2,}/g, " ").trim();
}

/**
 * Profilo con cui riprendere un tab: quello salvato; per le sessioni vecchie
 * (senza `agentId`) quello che riconosce il comando, poi il predefinito.
 */
function resumeProfile(tab: { agentId?: string }, original: string, profiles: RestoreProfileLike[]) {
  if (tab.agentId !== undefined) return profiles.find((p) => p.agent_id === tab.agentId);
  return profileForCommand(original, profiles) ?? profiles.find((p) => p.is_default);
}

/**
 * Comando da eseguire quando un tab viene ripristinato, guidato dal contratto
 * di ripresa (`session_resume`) dell'integrazione del tab:
 * - con `agentSessionId` (valido) e comando dell'agente: il comando originale
 *   senza i flag di ripresa, inserito nel modello di ripresa;
 * - con `agentSessionId` ma senza comando dell'agente (shell dove l'utente ha
 *   lanciato l'agente a mano): il modello con il nome canonico del programma;
 * - altrimenti il comando originale ("" = shell semplice), anche se
 *   l'integrazione non dichiara la ripresa.
 * Il risultato non va passato da `sanitizeLaunchCommand` (1000 caratteri).
 */
export function restoreCommand(
  tab: { command: string; agentId?: string; agentSessionId?: string },
  profiles: RestoreProfileLike[],
): string {
  const original = (tab.command ?? "").trim();
  const sid = tab.agentSessionId;
  if (!isSafeAgentSessionId(sid)) return original;
  const resume = resumeProfile(tab, original, profiles)?.session_resume;
  if (!resume) return original;
  // Il comando di base si limita PRIMA di applicare il modello: il risultato
  // puo' superare COMMAND_MAX e non va ritagliato (spezzerebbe l'id).
  const command = isResumeCommand(resume, original)
    ? stripResumeFlags(original, resume.strip_flags).slice(0, COMMAND_MAX).trim()
    : resume.command_names[0];
  // Un'unica passata: un segnaposto dentro il comando non viene riespanso e
  // `$` non ha significato speciale.
  return resume.resume_template.replace(/\{command\}|\{id\}/g, (placeholder) =>
    placeholder === "{command}" ? command : sid,
  );
}
