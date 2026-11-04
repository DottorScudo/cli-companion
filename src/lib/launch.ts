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

/** Tutti gli avvii di un workspace: prima gli integrati, poi i suoi. */
export function allLaunches<T extends Launch, B extends Launch>(ws: { launches?: T[] }, builtins: B[]): (T | B)[] {
  return [...builtins, ...(ws.launches ?? [])];
}

/**
 * Avvio predefinito effettivo: `default_launch` del workspace se punta a un
 * avvio noto ("shell" = nessuno), altrimenti (assente o id sparito) quello
 * globale, che deve essere un integrato noto; altrimenti nessuno (shell vuota).
 */
export function resolveDefaultLaunch<T extends Launch, B extends Launch>(
  ws: { launches?: T[] } & Pick<Workspace, "default_launch">,
  builtins: B[],
  globalDefault: string | null | undefined,
): T | B | undefined {
  const own = ws.default_launch;
  if (own === SHELL_LAUNCH_ID) return undefined;
  if (own) {
    const found = allLaunches(ws, builtins).find((l) => l.id === own);
    if (found) return found;
  }
  if (globalDefault) return builtins.find((b) => b.id === globalDefault);
  return undefined;
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
