import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  agentEvent,
  automationUpdate,
  createRuntimeStore,
  remove,
  sameStatus,
  seed,
  setBusy,
  setTitle,
  statusOf,
  userInput,
} from "../src/stores/tabRuntime.ts";

const automation = { title: "Task", status: "running", agentId: "claude-code" };
const withTab = (init) => seed({}, "a", init);

describe("tabRuntime: seed e remove", () => {
  it("seed crea la voce con busy falso e ignora i campi undefined", () => {
    const s = seed({}, "a", { title: "vim", agent: undefined });
    assert.deepEqual(s.a, { busy: false, title: "vim" });
    assert.deepEqual(seed({}, "b").b, { busy: false });
  });

  it("seed su una voce esistente non la tocca", () => {
    const s = withTab({ title: "uno" });
    assert.equal(seed(s, "a", { title: "due" }), s);
  });

  it("remove toglie la voce; chiave assente = stesso stato", () => {
    const s = withTab();
    assert.deepEqual(remove(s, "a"), {});
    assert.equal(remove(s, "x"), s);
  });
});

describe("tabRuntime: nessuna modifica = stesso stato", () => {
  it("setTitle e setBusy con lo stesso valore", () => {
    const s = withTab({ title: "t" });
    assert.equal(setTitle(s, "a", "t"), s);
    assert.equal(setBusy(s, "a", false), s);
    assert.equal(setTitle(s, "a", "nuovo").a.title, "nuovo");
    assert.equal(setBusy(s, "a", true).a.busy, true);
  });

  it("chiave sconosciuta: nessun effetto per tutte le azioni", () => {
    const s = withTab();
    assert.equal(setTitle(s, "x", "t"), s);
    assert.equal(setBusy(s, "x", true), s);
    assert.equal(agentEvent(s, "x", { event: "finished" }), s);
    assert.equal(userInput(s, "x", true), s);
    assert.equal(automationUpdate(s, "x", { status: "idle", title: "t" }), s);
  });

  it("evento sconosciuto (test) o input su un tab non agente", () => {
    const s = withTab();
    assert.equal(agentEvent(s, "a", { event: "test" }), s);
    assert.equal(agentEvent(s, "a", { event: "session-end" }), s);
    assert.equal(userInput(s, "a", true), s);
  });

  it("userInput: Invio durante running e input senza ✓/❗ non cambiano nulla", () => {
    const running = withTab({ agent: { phase: "running" } });
    assert.equal(userInput(running, "a", true), running);
    const idle = withTab({ agent: { phase: "idle" } });
    assert.equal(userInput(idle, "a", false), idle);
  });

  it("automationUpdate: dati uguali o tab non automatico", () => {
    const s = withTab({ automation });
    assert.equal(automationUpdate(s, "a", { status: "running", title: "Task" }), s);
    assert.equal(automationUpdate(withTab(), "a", { status: "idle", title: "T" }).a.automation, undefined);
  });
});

describe("tabRuntime: transizioni", () => {
  it("agentEvent: session-start rende il tab un tab-agente e conserva gli id", () => {
    let s = withTab();
    s = agentEvent(s, "a", { event: "session-start", agent: "claude-code", agent_session_id: "s-1" });
    assert.deepEqual(s.a.agent, { phase: "idle", agentSessionId: "s-1", agentId: "claude-code" });
    s = agentEvent(s, "a", { event: "needs-input" });
    assert.equal(s.a.agent.phase, "needs-input");
    assert.equal(s.a.agent.agentSessionId, "s-1");
  });

  it("userInput: Invio => running, altro input toglie ✓", () => {
    let s = withTab({ agent: { phase: "finished" } });
    assert.equal(userInput(s, "a", false).a.agent.phase, "idle");
    s = userInput(s, "a", true);
    assert.equal(s.a.agent.phase, "running");
  });

  it("automationUpdate applica stato e titolo", () => {
    const s = automationUpdate(withTab({ automation }), "a", {
      status: "completed",
      title: "Altro",
      last_report: "ok",
    });
    assert.deepEqual(s.a.automation, { ...automation, status: "completed", title: "Altro", lastReport: "ok" });
  });

  it("le altre voci non vengono toccate", () => {
    const s = setBusy(seed(seed({}, "a"), "b"), "a", true);
    assert.equal(s.b.busy, false);
    assert.deepEqual(Object.keys(s), ["a", "b"]);
  });
});

describe("statusOf / sameStatus", () => {
  it("senza voce: inattivo; busy: in esecuzione; l'agente sostituisce l'euristica", () => {
    assert.equal(statusOf(undefined).label, "Inattivo");
    assert.equal(statusOf({ busy: true }).label, "In esecuzione");
    assert.equal(statusOf({ busy: true, agent: { phase: "needs-input" } }).icon, "alert");
  });

  it("sameStatus confronta per valore, anche con null", () => {
    assert.ok(sameStatus(statusOf(undefined), statusOf({ busy: false })));
    assert.ok(!sameStatus(statusOf(undefined), statusOf({ busy: true })));
    assert.ok(sameStatus(null, null));
    assert.ok(!sameStatus(null, statusOf(undefined)));
    assert.ok(!sameStatus(statusOf(undefined), null));
  });
});

describe("createRuntimeStore", () => {
  it("le azioni a vuoto non notificano", () => {
    const store = createRuntimeStore();
    store.seed("a", { title: "t" });
    let calls = 0;
    store.subscribe(() => calls++);
    store.setTitle("a", "t");
    store.setBusy("a", false);
    store.setBusy("zzz", true);
    store.agentEvent("a", { event: "test" });
    store.userInput("a", true);
    store.automationUpdate("a", { status: "idle", title: "x" });
    store.remove("zzz");
    store.seed("a");
    assert.equal(calls, 0);
  });

  it("le azioni che cambiano notificano una volta e get legge subito", () => {
    const store = createRuntimeStore();
    let calls = 0;
    store.subscribe(() => calls++);
    store.seed("a");
    store.setBusy("a", true);
    assert.equal(calls, 2);
    assert.equal(store.get("a").busy, true);
    store.remove("a");
    assert.equal(store.get("a"), undefined);
    assert.equal(calls, 3);
  });
});
