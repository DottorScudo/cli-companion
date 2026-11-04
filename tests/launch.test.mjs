import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  LAUNCH_COMMAND_MAX,
  buildLaunchInput,
  SHELL_LAUNCH_ID,
  allLaunches,
  launchCommands,
  resolveDefaultLaunch,
  sanitizeLaunchCommand,
  shouldInjectLaunch,
  launchAgentId,
  unavailableDefaultLaunch,
  workspaceAgents,
} from "../src/lib/launch.ts";

describe("launch: comando di avvio", () => {
  it("trim + limite 1000 caratteri", () => {
    assert.equal(LAUNCH_COMMAND_MAX, 1000);
    assert.equal(sanitizeLaunchCommand("  npm run dev  "), "npm run dev");
    assert.equal(sanitizeLaunchCommand(""), "");
    assert.equal(sanitizeLaunchCommand("x".repeat(1500)), "x".repeat(1000));
  });

  it("vuoto o blank => nessuna iniezione", () => {
    assert.equal(shouldInjectLaunch(""), false);
    assert.equal(shouldInjectLaunch("   \t  "), false);
    assert.equal(shouldInjectLaunch("ls"), true);
  });

  it("l'input inviato al pty termina con ritorno carrello", () => {
    assert.equal(buildLaunchInput("ls"), "ls\r");
  });

  const a = { id: "a", name: "A", command: "ls" };
  const b = { id: "b", name: "B", command: "pwd" };
  const cc = {
    id: "builtin:claude-code",
    name: "Claude Code",
    command: "claude",
    restore_command: "claude --resume",
    agent_id: "claude-code",
  };
  const orch = { ...cc, id: "builtin:preset:task-intake", name: "Task Intake" };

  it("allLaunches: prima gli integrati, poi i personalizzati", () => {
    assert.deepEqual(allLaunches({ launches: [a, b] }, [cc]), [cc, a, b]);
    assert.deepEqual(allLaunches({}, []), []);
  });

  it("resolveDefaultLaunch: workspace > globale > nessuno", () => {
    const ws = (d) => ({ launches: [a, b], default_launch: d });
    assert.equal(resolveDefaultLaunch(ws("b"), [cc], "builtin:claude-code"), b);
    assert.equal(resolveDefaultLaunch(ws("builtin:preset:task-intake"), [cc, orch], null), orch);
    // "shell" forza la shell vuota anche con default globale.
    assert.equal(resolveDefaultLaunch(ws(SHELL_LAUNCH_ID), [cc], "builtin:claude-code"), undefined);
    // Ereditato: null/assente => globale.
    assert.equal(resolveDefaultLaunch(ws(null), [cc], "builtin:claude-code"), cc);
    assert.equal(resolveDefaultLaunch({ launches: [a] }, [cc], "builtin:claude-code"), cc);
    // Id sparito => ripiega sul globale.
    assert.equal(resolveDefaultLaunch(ws("gone"), [cc], "builtin:claude-code"), cc);
    // Globale sconosciuto o assente => nessuno.
    assert.equal(resolveDefaultLaunch(ws(null), [cc], "builtin:nope"), undefined);
    assert.equal(resolveDefaultLaunch(ws(null), [cc], null), undefined);
    assert.equal(resolveDefaultLaunch({}, [], undefined), undefined);
  });

  it("launchCommands: integrato salva restore_command, custom uguale", () => {
    assert.deepEqual(launchCommands(cc), { command: "claude --resume", startCommand: "claude" });
    assert.deepEqual(launchCommands(a), { command: "ls", startCommand: "ls" });
    assert.deepEqual(launchCommands(undefined), { command: "", startCommand: "" });
  });
});

describe("launch: avvii integrati non disponibili", () => {
  const cc = { id: "builtin:claude-code", name: "Claude Code", command: "claude" };
  const ws = (d) => ({ launches: [{ id: "l1", name: "L", command: "ls" }], default_launch: d });

  it("un integrato scelto dal workspace che non c'e' piu' non ripiega sul globale: shell", () => {
    assert.equal(resolveDefaultLaunch(ws("builtin:preset:task-intake@gone"), [cc], "builtin:claude-code"), undefined);
    assert.equal(resolveDefaultLaunch(ws("builtin:codex"), [cc], "builtin:claude-code"), undefined);
  });

  it("unavailableDefaultLaunch: l'id salvato che non c'e' piu', altrimenti null", () => {
    // Integrato del workspace sparito.
    assert.equal(unavailableDefaultLaunch(ws("builtin:codex"), [cc], null), "builtin:codex");
    // Globale sparito, workspace che eredita o con un avvio sparito.
    assert.equal(unavailableDefaultLaunch(ws(null), [cc], "builtin:codex"), "builtin:codex");
    assert.equal(unavailableDefaultLaunch(ws("gone"), [cc], "builtin:codex"), "builtin:codex");
    // Tutto a posto.
    assert.equal(unavailableDefaultLaunch(ws("builtin:claude-code"), [cc], "builtin:codex"), null);
    assert.equal(unavailableDefaultLaunch(ws("l1"), [cc], "builtin:codex"), null);
    assert.equal(unavailableDefaultLaunch(ws(null), [cc], "builtin:claude-code"), null);
    assert.equal(unavailableDefaultLaunch(ws(null), [cc], null), null);
    // Shell esplicita: nessun avviso nemmeno con un globale sparito.
    assert.equal(unavailableDefaultLaunch(ws(SHELL_LAUNCH_ID), [cc], "builtin:codex"), null);
    // Elenco non ancora letto: non si puo' dire.
    assert.equal(unavailableDefaultLaunch(ws("builtin:claude-code"), [], "builtin:claude-code"), null);
  });

  it("launchAgentId: l'agente di un integrato, niente per un avvio personalizzato", () => {
    assert.equal(launchAgentId({ ...cc, agent_id: "claude-code" }), "claude-code");
    assert.equal(launchAgentId({ id: "l1", name: "L", command: "ls" }), undefined);
    assert.equal(launchAgentId(undefined), undefined);
  });

  it("workspaceAgents: agenti propri senza doppioni e in ordine stabile", () => {
    assert.deepEqual(workspaceAgents([{ agent: "b" }, {}, { agent: "a" }, { agent: "b" }, { agent: undefined }]), [
      "a",
      "b",
    ]);
    assert.deepEqual(workspaceAgents([]), []);
  });
});
