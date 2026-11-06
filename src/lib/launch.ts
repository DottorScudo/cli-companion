// Avvii personalizzati per workspace: comando eseguito all'apertura di un
// terminale. File volutamente a zero dipendenze: importabile sia dall'app sia
// dai test `node --test` (type-stripping), come src/lib/terminal.ts.

import type { Launch } from "../bindings/Launch.ts";
import type { BuiltinLaunch } from "../bindings/BuiltinLaunch.ts";
import type { Workspace } from "../bindings/Workspace.ts";

/** Lunghezza massima del comando di un avvio. */
export const LAUNCH_COMMAND_MAX = 1000;

/** Lunghezza massima del nome di un avvio. */
export const LAUNCH_NAME_MAX = 80;

/** Attesa dopo il primo output del pty prima di inviare il comando. */
export const LAUNCH_AFTER_OUTPUT_MS = 150;

/**
 * Fallback: se la shell non ha ancora stampato nulla (avvio freddo WSL),
 * invia comunque il comando dopo questo tempo dallo spawn.
 */
export const LAUNCH_FALLBACK_MS = 2500;

/**
 * Normalizza il comando: trim, stringa vuota se blank, troncato a 1000
 * caratteri. Lo spacing interno è preservato (fa parte del comando).
 */
export function sanitizeLaunchCommand(raw: string): string {
  const trimmed = raw.trim();
  if (trimmed.length <= LAUNCH_COMMAND_MAX) return trimmed;
  return trimmed.slice(0, LAUNCH_COMMAND_MAX);
}

/** True solo se c'è un comando da eseguire (non vuoto/blank). */
export function shouldInjectLaunch(cmd: string): boolean {
  return cmd.trim() !== "";
}

/**
 * Costruisce l'input da scrivere nel pty: comando + ritorno carrello
 * (Invio). Funziona su PowerShell/cmd e sulle shell WSL perché simula
 * la digitazione dell'utente.
 */
export function buildLaunchInput(cmd: string): string {
  return `${cmd}\r`;
}

/** Valore di `default_launch` per la shell vuota. */
export const SHELL_LAUNCH_ID = "shell";

/** Un avvio che si può nominare come predefinito: del workspace, integrato o di un Bot. */
interface LaunchRef {
  id: string;
}

/** Tutti gli avvii di un workspace: prima quelli condivisi (integrati, Bot), poi i suoi. */
export function allLaunches<T extends LaunchRef, B extends LaunchRef>(ws: { launches?: T[] }, shared: B[]): (T | B)[] {
  return [...shared, ...(ws.launches ?? [])];
}

/** Prefissi degli id degli avvii condivisi da tutti i workspace: integrati e Bot. */
const SHARED_LAUNCH_PREFIXES = ["builtin:", "bot:"] as const;

const isSharedLaunchId = (id: string): boolean => SHARED_LAUNCH_PREFIXES.some((prefix) => id.startsWith(prefix));

/**
 * Avvio predefinito effettivo: `default_launch` del workspace se punta a un
 * avvio noto ("shell" = nessuno), altrimenti (assente o id sparito) quello
 * globale, che deve essere un avvio condiviso noto; altrimenti nessuno (shell
 * vuota). `shared` sono gli avvii integrati e dei Bot. Un condiviso scelto dal
 * workspace che non c'è più (integrazione disabilitata, Bot eliminato) non
 * ripiega sul globale: l'utente l'aveva scelto apposta, parte la shell.
 */
export function resolveDefaultLaunch<T extends LaunchRef, B extends LaunchRef>(
  ws: { launches?: T[] } & Pick<Workspace, "default_launch">,
  shared: B[],
  globalDefault: string | null | undefined,
): T | B | undefined {
  const own = ws.default_launch;
  if (own === SHELL_LAUNCH_ID) return undefined;
  if (own) {
    const found = allLaunches(ws, shared).find((l) => l.id === own);
    if (found) return found;
    if (isSharedLaunchId(own)) return undefined;
  }
  if (globalDefault) return shared.find((b) => b.id === globalDefault);
  return undefined;
}

/**
 * Id salvato come predefinito che punta a un avvio condiviso non più
 * disponibile (integrazione disabilitata o sconosciuta, Bot eliminato o
 * disattivato); `null` se non c'è. Con l'elenco dei condivisi ancora vuoto non
 * si può dire: `null`.
 */
export function unavailableDefaultLaunch<T extends LaunchRef, B extends LaunchRef>(
  ws: { launches?: T[] } & Pick<Workspace, "default_launch">,
  shared: B[],
  globalDefault: string | null | undefined,
): string | null {
  if (shared.length === 0) return null;
  const own = ws.default_launch;
  if (own === SHELL_LAUNCH_ID) return null;
  if (own) {
    if (allLaunches(ws, shared).some((l) => l.id === own)) return null;
    if (isSharedLaunchId(own)) return own;
  }
  return globalDefault && !shared.some((b) => b.id === globalDefault) ? globalDefault : null;
}

/** Agenti propri dei workspace, senza doppioni e in ordine stabile. */
export function workspaceAgents(list: readonly Pick<Workspace, "agent">[]): string[] {
  return [...new Set(list.flatMap((w) => (w.agent ? [w.agent] : [])))].sort();
}

/** Agente di un avvio integrato; `undefined` per un avvio personalizzato. */
export function launchAgentId(
  launch: (Launch & Partial<Pick<BuiltinLaunch, "agent_id">>) | undefined,
): string | undefined {
  return launch?.agent_id;
}

/**
 * Comandi di un avvio: `command` è ciò che si salva nel tab (per gli
 * integrati il `restore_command`), `startCommand` ciò che si esegue ora.
 */
export function launchCommands(launch: (Launch & Partial<Pick<BuiltinLaunch, "restore_command">>) | undefined): {
  command: string;
  startCommand: string;
} {
  if (!launch) return { command: "", startCommand: "" };
  if (launch.restore_command !== undefined) {
    return { command: launch.restore_command, startCommand: launch.command };
  }
  return { command: launch.command, startCommand: launch.command };
}
