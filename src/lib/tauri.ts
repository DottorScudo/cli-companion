import { invoke, type Channel } from "@tauri-apps/api/core";
import type { NotifyPayload as AgentEvent } from "../bindings/NotifyPayload.ts";
import type { AgentEnvStatus } from "../bindings/AgentEnvStatus.ts";
import type { AgentProfile } from "../bindings/AgentProfile.ts";
import type { TabEvent as AutomationClose } from "../bindings/TabEvent.ts";
import type { SpawnEvent as AutomationSpawn } from "../bindings/SpawnEvent.ts";
import type { AutomationTabInfo } from "../bindings/AutomationTabInfo.ts";
import type { BotSettings } from "../bindings/BotSettings.ts";
import type { BuiltinLaunch } from "../bindings/BuiltinLaunch.ts";
import type { Delivery } from "../bindings/Delivery.ts";
import type { E2eEvent as E2eUpdateEvent } from "../bindings/E2eEvent.ts";
import type { HubMessage } from "../bindings/HubMessage.ts";
import type { HubSettings } from "../bindings/HubSettings.ts";
import type { HubThread } from "../bindings/HubThread.ts";
import type { HubThreadSummary } from "../bindings/HubThreadSummary.ts";
import type { IntegrationEnv } from "../bindings/IntegrationEnv.ts";
import type { IntegrationInfo } from "../bindings/IntegrationInfo.ts";
import type { IntegrationsState } from "../bindings/IntegrationsState.ts";
import type { IpcStatus } from "../bindings/IpcStatus.ts";
import type { LaunchSettings } from "../bindings/LaunchSettings.ts";
import type { OutdatedPlugin } from "../bindings/OutdatedPlugin.ts";
import type { PermissionMode } from "../bindings/PermissionMode.ts";
import type { PluginUpdateResult } from "../bindings/PluginUpdateResult.ts";
import type { PtyEvent } from "../bindings/PtyEvent.ts";
import type { RepoConfig } from "../bindings/RepoConfig.ts";
import type { Schedule } from "../bindings/Schedule.ts";
import type { ScheduleState } from "../bindings/ScheduleState.ts";
import type { SessionUsage } from "../bindings/SessionUsage.ts";
import type { SettingsDecision } from "../bindings/SettingsDecision.ts";
import type { StoreWarning } from "../bindings/StoreWarning.ts";
import type { TargetKind } from "../bindings/TargetKind.ts";
import type { UsageLimitsReply } from "../bindings/UsageLimitsReply.ts";
import type { Workspace } from "../bindings/Workspace.ts";
import type { BoardView, E2eRun, NewTaskSpec, TaskAction, TaskNotifyEvent, TaskPatch } from "./board.ts";

// I tipi dei dati scambiati con il backend sono generati dal Rust (src/bindings):
// qui prendono i nomi usati dall'interfaccia.
export type {
  AgentEnvStatus,
  AgentEvent,
  AgentProfile,
  AutomationClose,
  AutomationSpawn,
  AutomationTabInfo,
  BotSettings,
  BuiltinLaunch,
  Delivery,
  HubMessage,
  HubSettings,
  HubThread,
  HubThreadSummary,
  IntegrationEnv,
  IntegrationInfo,
  IntegrationsState,
  OutdatedPlugin,
  PluginUpdateResult,
  PtyEvent,
  RepoConfig,
  SessionUsage,
  UsageLimitsReply,
  Workspace,
};
export type { DeliveryMode } from "../bindings/DeliveryMode.ts";
export type { E2eConfig } from "../bindings/E2eConfig.ts";
export type { Launch } from "../bindings/Launch.ts";
export type { MergeMethod } from "../bindings/MergeMethod.ts";

/** Argomenti di `add_workspace` (nessun DTO Rust: il comando prende i campi sciolti). */
export interface NewWorkspace {
  name: string;
  kind: TargetKind;
  path: string;
  distro?: string;
  shell?: string;
}

export const api = {
  listWorkspaces: () => invoke<Workspace[]>("list_workspaces"),

  addWorkspace: (w: NewWorkspace) =>
    invoke<Workspace>("add_workspace", {
      name: w.name,
      kind: w.kind,
      path: w.path,
      distro: w.distro ?? null,
      shell: w.shell && w.shell.trim() !== "" ? w.shell : null,
    }),

  renameWorkspace: (id: string, name: string) => invoke<Workspace>("rename_workspace", { id, name }),

  /** Agente dei worker del workspace (`null` = il predefinito globale). */
  setWorkspaceAgent: (id: string, agent: string | null) => invoke<Workspace>("set_workspace_agent", { id, agent }),
  setWorkerModel: (id: string, agent: string, model: string | null) =>
    invoke<Workspace>("set_worker_model", { id, agent, model }),
  setWorkerPermissionRules: (id: string, agent: string, allow: string[], deny: string[]) =>
    invoke<Workspace>("set_worker_permission_rules", { id, agent, allow, deny }),
  setWorkerPermission: (id: string, mode: PermissionMode) => invoke<Workspace>("set_worker_permission", { id, mode }),

  setWorkerRules: (workspaceId: string, rules: string) => invoke<Workspace>("set_worker_rules", { workspaceId, rules }),

  /** Modalita' di consegna dei task del workspace. */
  setDelivery: (id: string, delivery: Delivery) => invoke<Workspace>("set_delivery", { id, delivery }),

  /** La coda del workspace rispetta (o no) le fasce orarie globali. */
  setUseSchedule: (id: string, value: boolean) => invoke<Workspace>("set_use_schedule", { id, value }),

  /** Imposta (o con null rimuove) la configurazione di un repository del workspace. */
  setRepoConfig: (workspaceId: string, repo: string, config: RepoConfig | null) =>
    invoke<Workspace>("set_repo_config", { workspaceId, repo, config }),

  removeWorkspace: (id: string) => invoke<void>("remove_workspace", { id }),

  /** Nuovo ordine dei workspace visibili; il backend accoda gli altri. Restituisce l'elenco completo. */
  reorderWorkspaces: (ids: string[]) => invoke<Workspace[]>("reorder_workspaces", { ids }),

  setWorkspaceArchived: (workspaceId: string, archived: boolean) =>
    invoke<Workspace>("set_workspace_archived", { workspaceId, archived }),

  addLaunch: (workspaceId: string, name: string, command: string, isDefault: boolean) =>
    invoke<Workspace>("add_launch", { workspaceId, name, command, isDefault }),

  updateLaunch: (workspaceId: string, launchId: string, name: string, command: string) =>
    invoke<Workspace>("update_launch", { workspaceId, launchId, name, command }),

  setDefaultLaunch: (workspaceId: string, launchId: string | null) =>
    invoke<Workspace>("set_default_launch", { workspaceId, launchId }),

  /** Avvii integrati; `workspaceAgent` è l'agente proprio del workspace (`null` = nessuno). */
  builtinLaunches: (workspaceAgent: string | null) => invoke<BuiltinLaunch[]>("builtin_launches", { workspaceAgent }),

  getLaunchSettings: () => invoke<LaunchSettings>("get_launch_settings"),

  setGlobalDefaultLaunch: (launchId: string | null) =>
    invoke<LaunchSettings>("set_global_default_launch", { launchId }),

  removeLaunch: (workspaceId: string, launchId: string) =>
    invoke<Workspace>("remove_launch", { workspaceId, launchId }),

  openWorkspaceFolder: (id: string) => invoke<void>("open_workspace_folder", { id }),

  listWslDistros: () => invoke<string[]>("list_wsl_distros"),

  checkWslPath: (distro: string, path: string) => invoke<boolean>("check_wsl_path", { distro, path }),

  // Nota: Tauri v2 converte i nomi degli argomenti in camelCase
  // (workspace_id -> workspaceId): le chiavi qui devono esserlo già.
  ptySpawn: (
    workspaceId: string,
    tabId: string,
    cols: number,
    rows: number,
    onEvent: Channel<PtyEvent>,
    cwd?: string | null,
  ) => invoke<number>("pty_spawn", { workspaceId, tabId, cols, rows, onEvent, cwd: cwd ?? null }),

  ptyWrite: (sessionId: number, data: string) => invoke<void>("pty_write", { sessionId, data }),

  ptyResize: (sessionId: number, cols: number, rows: number) => invoke<void>("pty_resize", { sessionId, cols, rows }),

  ptyClose: (sessionId: number) => invoke<void>("pty_close", { sessionId }),

  testAgentNotify: () => invoke<string>("test_agent_notify"),

  ipcStatus: () => invoke<IpcStatus>("ipc_status"),

  /** Toast cliccabile (solo Windows): al click focus + `notification-activated`. */
  showAgentNotification: (tabId: string, title: string, body: string) =>
    invoke<void>("show_agent_notification", { tabId, title, body }),

  setShowUsage: (enabled: boolean) => invoke<LaunchSettings>("set_show_usage", { enabled }),

  getBotSettings: () => invoke<BotSettings>("get_bot_settings"),

  setBotSettings: (enabled: boolean, names: string[]) => invoke<BotSettings>("set_bot_settings", { enabled, names }),

  resetBotNames: () => invoke<BotSettings>("reset_bot_names"),

  setBotImage: (name: string, png: string, color: string) => invoke<BotSettings>("set_bot_image", { name, png, color }),

  removeBotImage: (name: string) => invoke<BotSettings>("remove_bot_image", { name }),

  // Agents HUB (backend: src-tauri/src/hub). `channel` = "general" o l'id di un workspace.
  hubGetSettings: () => invoke<HubSettings>("hub_get_settings"),

  hubSetEnabled: (enabled: boolean) => invoke<HubSettings>("hub_set_enabled", { enabled }),

  hubListThreads: (channel: string) => invoke<HubThreadSummary[]>("hub_list_threads", { channel }),

  hubGetThread: (channel: string, threadId: string) => invoke<HubThread>("hub_get_thread", { channel, threadId }),

  hubCreateThread: (channel: string, title: string) => invoke<HubThread>("hub_create_thread", { channel, title }),

  /** Messaggio dell'utente; `replyTo` = id di un messaggio dello stesso thread. */
  hubSend: (channel: string, threadId: string, body: string, replyTo: string | null = null) =>
    invoke<HubMessage>("hub_send", { channel, threadId, body, replyTo }),

  agentUsageLimits: (agentId: string, env: string) => invoke<UsageLimitsReply>("agent_usage_limits", { agentId, env }),

  agentSessionUsage: (agentId: string, env: string, transcriptPath: string) =>
    invoke<SessionUsage | null>("agent_session_usage", { agentId, env, transcriptPath }),

  agentProfiles: () => invoke<AgentProfile[]>("agent_profiles"),

  agentEnvStatus: () => invoke<AgentEnvStatus[]>("agent_env_status"),

  installAgentPlugin: (agentId: string, envKey: string) => invoke<string>("install_agent_plugin", { agentId, envKey }),

  agentPluginsOutdated: () => invoke<OutdatedPlugin[]>("agent_plugins_outdated"),

  agentPluginsUpdate: () => invoke<PluginUpdateResult[]>("agent_plugins_update"),

  removeAgentPlugin: (agentId: string, envKey: string) => invoke<string>("remove_agent_plugin", { agentId, envKey }),

  /** Tutte le integrazioni con stato e prontezza per target (backend: agents/integrations_commands.rs). */
  integrationsList: () => invoke<IntegrationInfo[]>("integrations_list"),

  setIntegrationEnabled: (id: string, enabled: boolean) =>
    invoke<IntegrationsState>("set_integration_enabled", { id, enabled }),

  setDefaultIntegration: (id: string) => invoke<IntegrationsState>("set_default_integration", { id }),

  // Sessioni automatiche (backend: src-tauri/src/automation/).
  /** Un tab automatico e' stato chiuso (utente, app o shell uscita). */
  automationTabClosed: (tabId: string) => invoke<void>("automation_tab_closed", { tabId }),

  automationSessions: () => invoke<AutomationTabInfo[]>("automation_sessions"),

  /** Listener registrati e sessione ripristinata: il timer dell'automazione puo' partire (idempotente). */
  frontendReady: () => invoke<void>("frontend_ready"),

  // Coda dei task (backend: src-tauri, contratto in docs/BOARD.MD).
  boardGet: (workspaceId: string) => invoke<BoardView>("board_get", { workspaceId }),

  /** Avvia a mano gli E2E di un repository ("." = root); ritorna l'esecuzione in corso. */
  e2eRunRepo: (workspaceId: string, repo: string) => invoke<E2eRun>("e2e_run_repo", { workspaceId, repo }),

  /** Ultimo esito manuale degli E2E di un repository, se c'è. */
  e2eLastResult: (workspaceId: string, repo: string) => invoke<E2eRun | null>("e2e_last_result", { workspaceId, repo }),

  /** Apre il report JUnit dell'ultima esecuzione E2E del task, o senza `taskId` di quella manuale del repository. */
  e2eOpenReport: (workspaceId: string, taskId?: string, repo?: string) =>
    invoke<void>("e2e_open_report", { workspaceId, taskId, repo }),

  /** Repository del workspace: "." se la root è un repo, più le sottocartelle con un .git. */
  workspaceDetectRepos: (workspaceId: string) => invoke<string[]>("workspace_detect_repos", { workspaceId }),

  boardSet: (workspaceId: string, settings: { maxParallel?: number; paused?: boolean }) =>
    invoke<BoardView>("board_set", {
      workspaceId,
      maxParallel: settings.maxParallel ?? null,
      paused: settings.paused ?? null,
    }),

  /** Task manuale (senza proposta); l'id lo genera il backend dal titolo. */
  taskCreate: (workspaceId: string, spec: NewTaskSpec) => invoke<BoardView>("task_create", { workspaceId, spec }),

  /** Solo i task Da fare / Falliti; `repo` e `model` a null tornano al valore predefinito. */
  taskUpdate: (workspaceId: string, taskId: string, patch: TaskPatch) =>
    invoke<BoardView>("task_update", { workspaceId, taskId, patch }),

  taskAction: (workspaceId: string, taskId: string, action: TaskAction) =>
    invoke<BoardView>("task_action", { workspaceId, taskId, action }),

  /** Riordina un task Da fare: prima di `beforeId`, o in fondo se assente. */
  taskMove: (workspaceId: string, taskId: string, beforeId?: string | null) =>
    invoke<BoardView>("task_move", { workspaceId, taskId, beforeId: beforeId ?? null }),

  /** Accetta o scarta una proposta di Task Intake. */
  proposalDecide: (workspaceId: string, proposalId: string, accept: boolean) =>
    invoke<BoardView>("proposal_decide", { workspaceId, proposalId, accept }),

  /** Applica o scarta una proposta di modifica alle impostazioni. */
  settingsProposalDecide: (workspaceId: string, proposalId: string, accept: boolean) =>
    invoke<SettingsDecision>("settings_proposal_decide", { workspaceId, proposalId, accept }),

  // Avvisi di persistenza (backend: src-tauri/src/store/warnings.rs); evento "store-warning".
  storeWarnings: () => invoke<StoreWarning[]>("store_warnings"),

  // Fasce orarie globali (solo come gate dello scheduler, docs/BOARD.MD).
  scheduleGet: () => invoke<ScheduleState>("schedule_get"),

  scheduleSet: (schedule: Schedule) => invoke<ScheduleState>("schedule_set", { schedule }),

  // Persistenza della sessione (backend: src-tauri/src/session.rs).
  loadSession: () => invoke<string | null>("load_session"),

  saveSession: (data: string) => invoke<void>("save_session", { data }),

  saveScrollback: (tabId: string, data: string) => invoke<void>("save_scrollback", { tabId, data }),

  loadScrollback: (tabId: string) => invoke<string | null>("load_scrollback", { tabId }),

  deleteScrollback: (tabId: string) => invoke<void>("delete_scrollback", { tabId }),

  pruneScrollback: (keep: string[]) => invoke<void>("prune_scrollback", { keep }),
};

/** Eventi emessi dal backend con il tipo del loro payload (usati da `useTauriEvent`). */
export interface TauriEvents {
  "notification-activated": AutomationClose;
  "automation-spawn": AutomationSpawn;
  "automation-close": AutomationClose;
  "automation-update": AutomationTabInfo;
  "schedule-state": ScheduleState;
  "board-update": BoardView;
  "e2e-update": E2eUpdateEvent;
  "task-notify": TaskNotifyEvent;
  "agent-event": AgentEvent;
  "integrations-changed": IntegrationsState;
  "store-warning": StoreWarning;
  "hub-update": HubThreadSummary;
}
