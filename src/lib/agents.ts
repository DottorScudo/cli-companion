// Coding agent: stato per ambiente (Impostazioni) e stato per tab (macchina
// a stati degli eventi degli hook). File volutamente a zero dipendenze:
// importabile sia dall'app sia dai test `node --test` (type-stripping).

import type { AgentEnvStatus } from "../bindings/AgentEnvStatus.ts";
import type { AgentInputEvent } from "../bindings/AgentInputEvent.ts";
import type { AgentProfile } from "../bindings/AgentProfile.ts";
import type { NotifyPayload } from "../bindings/NotifyPayload.ts";
import type { ResumeSpec } from "../bindings/ResumeSpec.ts";
import type { TargetKind } from "../bindings/TargetKind.ts";

// ---------------------------------------------------------------------------
// Impostazioni -> Coding agent
// ---------------------------------------------------------------------------

export type AgentEnvState = "not-found" | "not-installed" | "installed";

export function agentEnvState(env: Pick<AgentEnvStatus, "found" | "installed">): AgentEnvState {
  if (!env.found) return "not-found";
  return env.installed ? "installed" : "not-installed";
}

/** Testo di stato, con la versione se nota (anche per i non installati). */
export function agentEnvStateText(env: Pick<AgentEnvStatus, "found" | "installed" | "version">): string {
  const text = {
    installed: "Installato",
    "not-installed": "Non installato",
    "not-found": "Non trovato",
  }[agentEnvState(env)];
  const version = env.version?.trim();
  return env.found && version ? `${text} · v${version}` : text;
}

/** Etichetta del bottone principale: "Installa" / "Reinstalla" (null = nessuno). */
export function agentInstallLabel(env: Pick<AgentEnvStatus, "found" | "installed">): string | null {
  const state = agentEnvState(env);
  if (state === "not-found") return null;
  return state === "installed" ? "Reinstalla" : "Installa";
}

/** Testo del badge ambiente: `WIN` per Windows, il nome per le distro WSL. */
export function agentEnvBadge(env: Pick<AgentEnvStatus, "env_kind" | "env_label">): string {
  return env.env_kind === "windows" ? "WIN" : env.env_label;
}

export interface AgentGroup<T extends AgentEnvStatus = AgentEnvStatus> {
  agentId: string;
  agentName: string;
  envs: T[];
}

/** Raggruppa per agente mantenendo l'ordine di prima comparsa. */
export function groupByAgent<T extends AgentEnvStatus>(envs: T[]): AgentGroup<T>[] {
  const groups: AgentGroup<T>[] = [];
  for (const env of envs) {
    let group = groups.find((g) => g.agentId === env.agent_id);
    if (!group) {
      group = { agentId: env.agent_id, agentName: env.agent_name, envs: [] };
      groups.push(group);
    }
    group.envs.push(env);
  }
  return groups;
}

/** Chiave unica (agente, env) per lo stato "operazione in corso". */
export function agentRowKey(env: Pick<AgentEnvStatus, "agent_id" | "env_key">): string {
  return `${env.agent_id}::${env.env_key}`;
}

// ---------------------------------------------------------------------------
// Stato dell'agente in un tab
// ---------------------------------------------------------------------------

export type AgentPhase = "idle" | "running" | "finished" | "needs-input";

export interface AgentTabState {
  phase: AgentPhase;
  /** Id di sessione dell'agente (per riprendere la sessione), se noto. */
  agentSessionId?: string;
  /** Id del provider dell'agente, se noto. */
  agentId?: string;
  /** Percorso del transcript della sessione (come lo vede l'agente); non si salva. */
  transcriptPath?: string;
  /**
   * `true` dopo `session-end`: l'agente e' uscito, il tab torna ad avere
   * l'euristica busy/idle ma conserva `agentSessionId` per la ripresa.
   */
  ended?: boolean;
}

/** Campi dell'evento `agent-event` usati dalla macchina a stati. */
export type AgentEventLike = Pick<NotifyPayload, "event"> &
  Partial<Pick<NotifyPayload, "agent" | "agent_session_id" | "transcript_path">>;

/**
 * Applica un evento degli hook allo stato del tab. `session-start` marca il
 * tab come tab-agente (idle); `session-end` lo riporta a non-agente (con
 * `ended`, conservando l'id di sessione); `finished`/`needs-input` valgono anche se il
 * session-start e' sfuggito. Eventi sconosciuti (es. `test`): stato invariato.
 * L'id di sessione si aggiorna a ogni evento che lo porta.
 */
export function applyAgentEvent(prev: AgentTabState | undefined, ev: AgentEventLike): AgentTabState | undefined {
  if (ev.event === "session-end") {
    if (!prev) return prev;
    return { ...prev, phase: "idle", ended: true };
  }
  const phase: AgentPhase | null =
    ev.event === "session-start"
      ? "idle"
      : ev.event === "finished"
        ? "finished"
        : ev.event === "needs-input"
          ? "needs-input"
          : null;
  if (phase === null) return prev;
  const sid = ev.agent_session_id?.trim() || prev?.agentSessionId;
  const agentId = ev.agent?.trim() || prev?.agentId;
  const transcript = ev.transcript_path?.trim() || prev?.transcriptPath;
  const next: AgentTabState = { phase };
  if (sid) next.agentSessionId = sid;
  if (agentId) next.agentId = agentId;
  if (transcript) next.transcriptPath = transcript;
  return next;
}

/**
 * Input dell'utente nel terminale: Invio => running; qualsiasi altro input
 * toglie ✓/❗ riportando a idle. Tab non-agente: invariato. Ritorna lo stesso
 * oggetto se nulla cambia (per evitare re-render).
 */
export function applyUserInput(prev: AgentTabState | undefined, enter: boolean): AgentTabState | undefined {
  if (!prev || prev.ended) return prev;
  if (enter) return prev.phase === "running" ? prev : { ...prev, phase: "running" };
  if (prev.phase === "finished" || prev.phase === "needs-input") {
    return { ...prev, phase: "idle" };
  }
  return prev;
}

/**
 * `true` se i dati di `onData` di xterm sono una risposta automatica del
 * terminale (focus in/out, cursor position report, device attributes,
 * risposte OSC) e non un tasto premuto dall'utente.
 */
export function isTerminalReply(data: string): boolean {
  return (
    /^\x1b\[[IO]$/.test(data) ||
    /^\x1b\[[?>]?[0-9;]*[Rc]$/.test(data) ||
    /^\x1b\[[?]?[0-9;]*\$y$/.test(data) ||
    /^\x1b\][\s\S]*(\x07|\x1b\\)$/.test(data)
  );
}

export type TabStatusIcon = "dot" | "check" | "alert";
export type TabStatusTone = "busy" | "idle" | "done" | "attention";

export interface TabStatus {
  icon: TabStatusIcon;
  tone: TabStatusTone;
  /** Testo per aria-label/title. */
  label: string;
}

/** Cosa mostrare nel tab: per i tab-agente lo stato sostituisce l'euristica busy/idle. */
export function tabStatus(agent: AgentTabState | undefined, busy: boolean): TabStatus {
  if (!agent || agent.ended) {
    return busy
      ? { icon: "dot", tone: "busy", label: "In esecuzione" }
      : { icon: "dot", tone: "idle", label: "Inattivo" };
  }
  switch (agent.phase) {
    case "running":
      return { icon: "dot", tone: "busy", label: "Agente al lavoro" };
    case "idle":
      return { icon: "dot", tone: "idle", label: "Agente inattivo" };
    case "finished":
      return { icon: "check", tone: "done", label: "Agente ha finito" };
    case "needs-input":
      return { icon: "alert", tone: "attention", label: "Agente aspetta una risposta" };
  }
}

// ---------------------------------------------------------------------------
// Avvisi
// ---------------------------------------------------------------------------

export type AlertSound = "ding" | "attention";

/** Suono per un evento: `null` = nessuno (es. session-start). */
export function soundForEvent(event: string): AlertSound | null {
  if (event === "finished" || event === "test") return "ding";
  if (event === "needs-input") return "attention";
  return null;
}

/** Solo fine lavoro e richiesta di risposta meritano la notifica di sistema. */
export function shouldSystemNotify(event: string): boolean {
  return event === "finished" || event === "needs-input";
}

/** Testo della notifica di sistema: "<tab>: ha finito" / "<tab>: aspetta una risposta". */
export function notificationBody(event: string, tabLabel: string, message?: string | null): string {
  if (event === "needs-input") {
    const m = message?.trim();
    return `${tabLabel}: ${m ? m : "aspetta una risposta"}`;
  }
  return `${tabLabel}: ha finito`;
}

// ---------------------------------------------------------------------------
// Profili di input (vedi `InputProfile` in agents/mod.rs)
// ---------------------------------------------------------------------------

/** Campi di `AgentProfile` usati dal resolver. */
export type AgentProfileLike = Pick<AgentProfile, "agent_id" | "input_profile">;

/**
 * Sequenza da inviare al pty al posto del comportamento standard per
 * `event`, o `null` se l'agente non ha una regola per questo ambiente
 * (`env_kind` assente nella regola = qualunque ambiente).
 */
export function resolveInputOverride(
  profiles: AgentProfileLike[],
  agentId: string | null | undefined,
  envKind: TargetKind,
  event: AgentInputEvent,
): string | null {
  if (!agentId) return null;
  const rules = profiles.find((p) => p.agent_id === agentId)?.input_profile.rules ?? [];
  const rule = rules.find((r) => r.event === event && (r.env_kind === null || r.env_kind === envKind));
  return rule ? rule.send : null;
}

/** Campi di `AgentProfile` usati per riconoscere il comando di un agente. */
export type CommandProfileLike = Pick<AgentProfile, "agent_id" | "session_resume">;

/** `true` se il primo token del comando e' uno dei nomi del programma (senza distinguere le maiuscole). */
export function isResumeCommand(resume: Pick<ResumeSpec, "command_names">, command: string): boolean {
  const program = command.trim().split(/\s+/)[0]?.toLowerCase() ?? "";
  return resume.command_names.some((name) => name.toLowerCase() === program);
}

/** Integrazione che riconosce il comando di avvio, se ce n'e' una. */
export function profileForCommand<P extends CommandProfileLike>(
  command: string | null | undefined,
  profiles: P[],
): P | null {
  const text = command ?? "";
  return profiles.find((p) => p.session_resume !== null && isResumeCommand(p.session_resume, text)) ?? null;
}

/** Agente riconosciuto dal primo token di un comando di avvio (senza plugin). */
export function agentIdForCommand(command: string | null | undefined, profiles: CommandProfileLike[]): string | null {
  return profileForCommand(command, profiles)?.agent_id ?? null;
}

/**
 * Agente in esecuzione in un tab. Prima lo stato del plugin (da
 * `session-start` a `session-end`: dopo la fine l'agente e' uscito e il tab
 * e' una shell); senza plugin si ripiega sul comando con cui il tab e' stato
 * aperto/ripristinato.
 */
export function tabAgentId(
  tab: {
    agent?: Pick<AgentTabState, "agentId" | "ended">;
    startCommand?: string;
  },
  profiles: CommandProfileLike[],
): string | null {
  if (tab.agent?.ended) return null;
  return tab.agent?.agentId ?? agentIdForCommand(tab.startCommand, profiles);
}

// ---------------------------------------------------------------------------
// Barra di stato
// ---------------------------------------------------------------------------

/**
 * Riepilogo degli ambienti con il plugin installato, per la barra di stato:
 * "Nome agente · Windows, Ubuntu-24.04". Più agenti sono separati da "; ".
 * Stringa vuota se non c'è nulla di installato.
 */
export function installedAgentsSummary(
  envs: Array<Pick<AgentEnvStatus, "agent_id" | "agent_name" | "env_label" | "found" | "installed">>,
): string {
  const byAgent = new Map<string, { name: string; labels: string[] }>();
  for (const env of envs) {
    if (!env.found || !env.installed) continue;
    const entry = byAgent.get(env.agent_id) ?? { name: env.agent_name, labels: [] };
    entry.labels.push(env.env_label);
    byAgent.set(env.agent_id, entry);
  }
  return [...byAgent.values()].map((a) => `${a.name} · ${a.labels.join(", ")}`).join("; ");
}

/** Quanti terminali aspettano una risposta (❗). */
export function countNeedingInput(statuses: Array<{ icon: TabStatusIcon }>): number {
  return statuses.filter((s) => s.icon === "alert").length;
}

/** "1 terminale" / "N terminali". */
export function terminalCountText(n: number): string {
  return n === 1 ? "1 terminale" : `${n} terminali`;
}

/** Stato mostrato su un gruppo di terminali (tab della barra del titolo, riga della sidebar). */
export const GROUP_STATUS: Record<"alert" | "check" | "busy", TabStatus> = {
  alert: { icon: "alert", tone: "attention", label: "Un terminale aspetta una risposta" },
  check: { icon: "check", tone: "done", label: "Un agente ha finito" },
  busy: { icon: "dot", tone: "busy", label: "Terminali al lavoro" },
};
