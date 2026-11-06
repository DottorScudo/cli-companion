import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { currentLaunchId } from "../src/lib/legacyLaunch.ts";
import { parseSession, restoreCommand } from "../src/lib/session.ts";

describe("vecchie voci dei preset", () => {
  it("ogni preset va al suo Bot, l'orchestratore all'Intake", () => {
    assert.equal(currentLaunchId("builtin:preset:task-intake"), "bot:intake");
    assert.equal(currentLaunchId("builtin:preset:project-setup"), "bot:setup");
    assert.equal(currentLaunchId("builtin:preset:skill-wizard"), "bot:skill-wizard");
    assert.equal(currentLaunchId("builtin:orchestrator"), "bot:intake");
  });

  it("con l'agente scelto va al Bot derivato; un agente malformato ripiega sul predefinito", () => {
    assert.equal(currentLaunchId("builtin:preset:task-intake@opencode"), "bot:intake-opencode");
    assert.equal(currentLaunchId("builtin:preset:skill-wizard@my-agent"), "bot:skill-wizard-my-agent");
    for (const id of [
      "builtin:preset:task-intake@",
      "builtin:preset:task-intake@A b",
      "builtin:preset:task-intake@a@b",
    ]) {
      assert.equal(currentLaunchId(id), "bot:intake", id);
    }
  });

  it("gli altri id restano", () => {
    for (const id of ["l1", "shell", "builtin:claude-code", "bot:general", "builtin:preset:gone", "builtin:preset:"]) {
      assert.equal(currentLaunchId(id), id);
    }
  });
});

describe("sessione salvata con una vecchia voce di un preset", () => {
  const raw = JSON.stringify({
    version: 1,
    tabs: [
      {
        tabId: "a",
        workspaceId: "w1",
        command: "claude",
        launchId: "builtin:preset:task-intake",
        agentId: "claude-code",
      },
      { tabId: "b", workspaceId: "w1", command: "claude", launchId: "builtin:preset:project-setup@claude-code" },
    ],
  });

  it("si legge senza errori, con la voce del Bot e il comando di ripristino salvato", () => {
    const parsed = parseSession(raw, ["w1"]);
    assert.ok(parsed);
    assert.deepEqual(
      parsed.tabs.map((t) => [t.launchId, t.command]),
      [
        ["bot:intake", "claude"],
        ["bot:setup-claude-code", "claude"],
      ],
    );
    assert.equal(restoreCommand(parsed.tabs[0], []), "claude");
  });
});
