import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  formatTokens,
  limitSeverity,
  formatReset,
  limitItems,
  limitGroups,
  limitGroupText,
  usageLimitsNotices,
  usageTargets,
  usageText,
  usageTooltip,
  shouldRefreshSessionUsage,
  sessionPollInterval,
  workspaceEnvKey,
  LIMITS_POLL_MS,
  SESSION_POLL_MS,
} from "../src/lib/usage.ts";

// Date locali (senza "Z"): i test non dipendono dal fuso orario.
const now = new Date(2026, 9, 21, 10, 0, 0);
const iso = (y, m, d, h, min) => new Date(y, m - 1, d, h, min).toISOString();

const totals = (over = {}) => ({
  input_tokens: 0,
  output_tokens: 0,
  cache_read_tokens: 0,
  cache_creation_tokens: 0,
  ...over,
});

describe("usage: formato dei token", () => {
  it("numeri compatti", () => {
    assert.equal(formatTokens(0), "0");
    assert.equal(formatTokens(950), "950");
    assert.equal(formatTokens(1000), "1k");
    assert.equal(formatTokens(1234), "1,2k");
    assert.equal(formatTokens(12_345), "12k");
    assert.equal(formatTokens(999_600), "1M");
    assert.equal(formatTokens(1_234_567), "1,2M");
    assert.equal(formatTokens(12_345_678), "12M");
  });
});

describe("usage: gravita e reset", () => {
  it("soglie", () => {
    assert.equal(limitSeverity(0), "normal");
    assert.equal(limitSeverity(79.9), "normal");
    assert.equal(limitSeverity(80), "warning");
    assert.equal(limitSeverity(99), "warning");
    assert.equal(limitSeverity(100), "critical");
  });

  it("oggi ora, altrimenti data, null = niente", () => {
    assert.equal(formatReset(iso(2026, 10, 21, 14, 5), now), "14:05");
    assert.equal(formatReset(iso(2026, 10, 28, 9, 0), now), "28/10");
    assert.equal(formatReset(null, now), null);
    assert.equal(formatReset("boh", now), null);
  });
});

describe("usage: integrazioni con i limiti", () => {
  const profile = (agent_id, agent_name, capabilities, usage_limits_notice = null) => ({
    agent_id,
    agent_name,
    capabilities,
    usage_limits_notice,
  });
  const env = (agent_id, env_key, found = true, installed = true) => ({ agent_id, env_key, found, installed });

  const profiles = [
    profile("claude-code", "Claude Code", ["hook-events", "usage-limits"], "Endpoint non documentato."),
    profile("foo", "Foo", ["usage-limits"], "Endpoint non documentato."),
    profile("bar", "Bar", ["hook-events"]),
  ];

  it("coppie agente/ambiente installate, solo per chi dichiara usage-limits", () => {
    const envs = [
      env("claude-code", "windows"),
      env("claude-code", "Ubuntu"),
      env("foo", "windows"),
      env("bar", "windows"),
      env("unknown", "windows"),
    ];
    const targets = usageTargets(profiles, envs);
    assert.deepEqual(
      targets.map((t) => [t.key, t.agentName]),
      [
        ["claude-code::windows", "Claude Code"],
        ["claude-code::Ubuntu", "Claude Code"],
        ["foo::windows", "Foo"],
      ],
    );
    assert.equal(targets[1].env, envs[1]);
  });

  it("ambienti non trovati o senza plugin: esclusi", () => {
    const envs = [env("foo", "windows", false, false), env("foo", "Ubuntu", true, false), env("foo", "Debian")];
    assert.deepEqual(
      usageTargets(profiles, envs).map((t) => t.key),
      ["foo::Debian"],
    );
    assert.deepEqual(usageTargets([], [env("foo", "windows")]), []);
    assert.deepEqual(usageTargets(profiles, []), []);
  });

  it("avvisi: uno per testo distinto, nell'ordine, senza vuoti", () => {
    assert.deepEqual(usageLimitsNotices(profiles), ["Endpoint non documentato."]);
    assert.deepEqual(
      usageLimitsNotices([profile("a", "A", [], "uno"), profile("b", "B", []), profile("c", "C", [], "due")]),
      ["uno", "due"],
    );
    assert.deepEqual(usageLimitsNotices([profile("a", "A", [], "")]), []);
    assert.deepEqual(usageLimitsNotices([]), []);
  });
});

describe("usage: barra di stato", () => {
  const ok = {
    state: "ok",
    five_hour: { used_percentage: 45, resets_at: iso(2026, 10, 21, 14, 20) },
    seven_day: { used_percentage: 2, resets_at: iso(2026, 10, 28, 9, 0) },
    message: null,
  };

  it("testo di un ambiente", () => {
    const [g] = limitGroups([{ key: "a::windows", agentName: "Agente", label: "WIN", reply: ok }], now);
    assert.equal(g.key, "a::windows");
    assert.equal(g.prefix, null);
    assert.equal(limitGroupText(g), "5h 45% · 14:20 │ sett. 2% · 28/10");
    assert.deepEqual(
      g.items.map((i) => i.severity),
      ["normal", "normal"],
    );
  });

  it("prefisso solo con piu' ambienti visibili", () => {
    const hidden = { state: "unavailable", five_hour: null, seven_day: null, message: "x" };
    const two = limitGroups(
      [
        { key: "a::windows", agentName: "Agente", label: "WIN", reply: ok },
        { key: "a::Ubuntu", agentName: "Agente", label: "Ubuntu", reply: ok },
        { key: "a::Debian", agentName: "Agente", label: "Debian", reply: hidden },
      ],
      now,
    );
    assert.deepEqual(
      two.map((g) => g.prefix),
      ["WIN", "Ubuntu"],
    );
    assert.ok(limitGroupText(two[1]).startsWith("Ubuntu 5h 45%"));
  });

  it("prefisso con più agenti: nome dell'agente e ambiente", () => {
    const hidden = { state: "unavailable", five_hour: null, seven_day: null, message: "x" };
    const groups = limitGroups(
      [
        { key: "a::windows", agentName: "Alfa", label: "WIN", reply: ok },
        { key: "b::windows", agentName: "Beta", label: "WIN", reply: ok },
        { key: "c::windows", agentName: "Gamma", label: "WIN", reply: hidden },
      ],
      now,
    );
    assert.deepEqual(
      groups.map((g) => [g.key, g.prefix]),
      [
        ["a::windows", "Alfa · WIN"],
        ["b::windows", "Beta · WIN"],
      ],
    );
    assert.ok(limitGroupText(groups[0]).startsWith("Alfa · WIN 5h 45%"));
  });

  it("un solo agente visibile: niente nome nel prefisso", () => {
    const hidden = { state: "unavailable", five_hour: null, seven_day: null, message: "x" };
    const groups = limitGroups(
      [
        { key: "a::windows", agentName: "Alfa", label: "WIN", reply: ok },
        { key: "a::Ubuntu", agentName: "Alfa", label: "Ubuntu", reply: ok },
        { key: "b::windows", agentName: "Beta", label: "WIN", reply: hidden },
      ],
      now,
    );
    assert.deepEqual(
      groups.map((g) => g.prefix),
      ["WIN", "Ubuntu"],
    );
  });

  it("scaduto: suggerimento muto con il nome dell'agente; altri stati nascosti", () => {
    const expired = { state: "expired", five_hour: null, seven_day: null, message: "login" };
    const [item] = limitItems(expired, "Alfa", now);
    assert.equal(item.text, "5h: apri Alfa");
    assert.equal(item.severity, "muted");
    assert.equal(item.title, "login");
    const noMessage = limitItems({ ...expired, message: null }, "Alfa", now);
    assert.equal(noMessage[0].title, "Accesso scaduto: apri Alfa per rinnovarlo.");
    for (const state of ["unavailable", "disabled", "unsupported"]) {
      assert.deepEqual(limitItems({ state, five_hour: null, seven_day: null, message: null }, "Alfa", now), []);
    }
    assert.deepEqual(limitItems(null, "Alfa", now), []);
  });

  it("gravita sulla finestra", () => {
    const items = limitItems({ ...ok, five_hour: { used_percentage: 100, resets_at: null } }, "Alfa", now);
    assert.equal(items[0].text, "5h 100%");
    assert.equal(items[0].severity, "critical");
  });
});

describe("usage: intestazione del pannello", () => {
  const u = {
    main: totals({
      input_tokens: 200,
      output_tokens: 45_000,
      cache_read_tokens: 1_000_000,
      cache_creation_tokens: 34_000,
    }),
    subagents: totals(),
    context_tokens: 82_000,
    model: "claude-opus",
    messages: 12,
  };

  it("testo compatto", () => {
    assert.equal(usageText(u), "↑ 1M ↓ 45k · ctx 82k");
  });

  it("tooltip: subagent solo se non zero", () => {
    const t = usageTooltip(u);
    assert.ok(t.includes("Cache letta: 1M"));
    assert.ok(t.includes("Modello: claude-opus"));
    assert.ok(t.includes("Messaggi: 12"));
    assert.ok(!t.includes("Subagent"));
    const t2 = usageTooltip({ ...u, subagents: totals({ input_tokens: 1500, output_tokens: 500 }) });
    assert.ok(t2.includes("Subagent: ↑ 1,5k ↓ 500"));
  });
});

describe("usage: polling", () => {
  it("costanti e regole", () => {
    assert.equal(LIMITS_POLL_MS, 180_000);
    assert.equal(sessionPollInterval("running"), SESSION_POLL_MS);
    assert.equal(sessionPollInterval("idle"), null);
    assert.equal(sessionPollInterval(undefined), null);
    for (const e of ["finished", "needs-input", "session-start"]) assert.ok(shouldRefreshSessionUsage(e));
    assert.ok(!shouldRefreshSessionUsage("test"));
  });

  it("chiave dell'ambiente", () => {
    assert.equal(workspaceEnvKey({ kind: "windows" }), "windows");
    assert.equal(workspaceEnvKey({ kind: "wsl", distro: "Ubuntu" }), "Ubuntu");
    assert.equal(workspaceEnvKey({ kind: "wsl", distro: null }), "windows");
  });
});
