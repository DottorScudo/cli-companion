import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  INITIAL_USAGE,
  clearLimits,
  clearSessions,
  createUsageStore,
  retainSessions,
  setLimit,
  setSession,
} from "../src/stores/usage.ts";
import {
  INITIAL_BOARDS,
  clearBoardError,
  createBoardsStore,
  setBoardError,
  storeBoard,
  taskCountsOf,
} from "../src/stores/boards.ts";
import { INITIAL_DRAG, createPaneDragStore, dragOver, endDrag, leaveDrag, startDrag } from "../src/stores/paneDrag.ts";

const limits = (pct) => ({
  state: "ok",
  five_hour: { used_percentage: pct, resets_at: null },
  seven_day: null,
  message: null,
});
const totals = (n) => ({ input_tokens: n, output_tokens: 0, cache_read_tokens: 0, cache_creation_tokens: 0 });
const usage = (n) => ({ main: totals(n), subagents: totals(0), context_tokens: n, model: null, messages: 1 });

describe("usage store", () => {
  it("setLimit con una risposta uguale per valore non cambia lo stato", () => {
    const s = setLimit(INITIAL_USAGE, "claude::win", limits(10));
    assert.equal(setLimit(s, "claude::win", limits(10)), s);
    assert.notEqual(setLimit(s, "claude::win", limits(20)), s);
    assert.equal(setLimit(s, "claude::win", limits(20)).limits["claude::win"].five_hour.used_percentage, 20);
  });

  it("setSession: uguale per valore = stesso stato; null toglie la voce", () => {
    const s = setSession(INITIAL_USAGE, "a", usage(5));
    assert.equal(setSession(s, "a", usage(5)), s);
    assert.equal(setSession(s, "a", usage(6)).sessions.a.context_tokens, 6);
    assert.deepEqual(setSession(s, "a", null).sessions, {});
    assert.equal(setSession(s, "zzz", null), s);
  });

  it("clearLimits e clearSessions: vuoti = stesso stato", () => {
    assert.equal(clearLimits(INITIAL_USAGE), INITIAL_USAGE);
    assert.equal(clearSessions(INITIAL_USAGE), INITIAL_USAGE);
    const s = setSession(setLimit(INITIAL_USAGE, "k", limits(1)), "a", usage(1));
    assert.deepEqual(clearLimits(s).limits, {});
    assert.equal(clearLimits(s).sessions, s.sessions);
    assert.deepEqual(clearSessions(s).sessions, {});
  });

  it("retainSessions tiene solo i tab vivi; se sono già tutti vivi non cambia", () => {
    const s = setSession(setSession(INITIAL_USAGE, "a", usage(1)), "b", usage(2));
    assert.equal(retainSessions(s, ["a", "b", "c"]), s);
    assert.deepEqual(Object.keys(retainSessions(s, new Set(["b"])).sessions), ["b"]);
    assert.equal(retainSessions(INITIAL_USAGE, []), INITIAL_USAGE);
  });

  it("lo store non notifica per risposte uguali", () => {
    const store = createUsageStore();
    store.setLimit("k", limits(1));
    store.setSession("a", usage(1));
    let calls = 0;
    store.subscribe(() => calls++);
    store.setLimit("k", limits(1));
    store.setSession("a", usage(1));
    store.setSession("zzz", null);
    store.retainSessions(["a"]);
    assert.equal(calls, 0);
    store.clearLimits();
    assert.equal(calls, 1);
  });
});

const board = (workspace_id, rev, tasks = []) => ({
  workspace_id,
  rev,
  max_parallel: 3,
  paused: false,
  updated_at: rev,
  tasks,
  proposals: [],
  settings_proposals: [],
  schedule_blocked: false,
  delivery: { mode: "commit", parallel: false, merge_method: "squash" },
});

describe("boards store", () => {
  it("storeBoard memorizza una board nuova per workspace", () => {
    const b = board("w1", 1);
    const s = storeBoard(INITIAL_BOARDS, b);
    assert.equal(s.boards.w1, b);
  });

  it("una board più vecchia o con la stessa rev non sostituisce né cambia lo stato", () => {
    const current = board("w1", 5);
    const s = storeBoard(INITIAL_BOARDS, current);
    assert.equal(storeBoard(s, board("w1", 4)), s);
    assert.equal(storeBoard(s, board("w1", 5)), s);
    const newer = board("w1", 6);
    assert.equal(storeBoard(s, newer).boards.w1, newer);
  });

  it("setBoardError / clearBoardError", () => {
    const s = setBoardError(INITIAL_BOARDS, "w1", "boom");
    assert.equal(s.errors.w1, "boom");
    assert.equal(setBoardError(s, "w1", "boom"), s);
    assert.equal(setBoardError(s, "w1", "altro").errors.w1, "altro");
    assert.deepEqual(clearBoardError(s, "w1").errors, {});
    assert.equal(clearBoardError(s, "w2"), s);
  });

  it("taskCountsOf conta per workspace ed è memoizzato per oggetto boards", () => {
    const tasks = [{ status: "queued" }, { status: "running", attention: true }, { status: "done" }];
    const s = storeBoard(storeBoard(INITIAL_BOARDS, board("w1", 1, tasks)), board("w2", 1));
    const counts = taskCountsOf(s.boards);
    assert.equal(counts, taskCountsOf(s.boards));
    assert.equal(counts.w1.total, 3);
    assert.equal(counts.w1.queued, 1);
    assert.equal(counts.w1.running, 1);
    assert.equal(counts.w1.attention, 1);
    assert.equal(counts.w2.total, 0);
    assert.notEqual(taskCountsOf(storeBoard(s, board("w3", 1)).boards), counts);
  });

  it("gli errori non invalidano i contatori", () => {
    const s = storeBoard(INITIAL_BOARDS, board("w1", 1));
    assert.equal(setBoardError(s, "w1", "x").boards, s.boards);
  });

  it("lo store non notifica per board vecchie o errori ripetuti", () => {
    const store = createBoardsStore();
    store.store(board("w1", 2));
    store.setError("w1", "boom");
    let calls = 0;
    store.subscribe(() => calls++);
    store.store(board("w1", 1));
    store.setError("w1", "boom");
    store.clearError("w2");
    assert.equal(calls, 0);
    store.clearError("w1");
    assert.equal(calls, 1);
  });
});

describe("paneDrag", () => {
  const panes = {
    a: { groupWorkspaceId: "w1" },
    b: { groupWorkspaceId: "w1" },
    c: { groupWorkspaceId: "w2" },
  };

  it("start imposta l'origine e azzera il bersaglio", () => {
    const s = startDrag(INITIAL_DRAG, "a");
    assert.deepEqual(s, { from: "a", over: null });
    assert.equal(startDrag(s, "a"), s);
  });

  it("dragOver: bersaglio solo con origine, pannello diverso e stesso gruppo", () => {
    const s = startDrag(INITIAL_DRAG, "a");
    assert.equal(dragOver(s, "b", panes).over, "b");
    assert.equal(dragOver(s, "a", panes), s, "su se stesso");
    assert.equal(dragOver(s, "c", panes), s, "altro gruppo");
    assert.equal(dragOver(INITIAL_DRAG, "b", panes), INITIAL_DRAG, "senza origine");
  });

  it("dragOver verso un pannello non ammesso spegne il bersaglio precedente", () => {
    const over = dragOver(startDrag(INITIAL_DRAG, "a"), "b", panes);
    assert.deepEqual(dragOver(over, "c", panes), { from: "a", over: null });
  });

  it("dragOver sullo stesso bersaglio = stesso stato", () => {
    const over = dragOver(startDrag(INITIAL_DRAG, "a"), "b", panes);
    assert.equal(dragOver(over, "b", panes), over);
  });

  it("leave spegne solo il bersaglio corrente", () => {
    const over = dragOver(startDrag(INITIAL_DRAG, "a"), "b", panes);
    assert.equal(leaveDrag(over, "c"), over);
    assert.deepEqual(leaveDrag(over, "b"), { from: "a", over: null });
  });

  it("end azzera tutto; già a riposo = stesso stato", () => {
    const over = dragOver(startDrag(INITIAL_DRAG, "a"), "b", panes);
    assert.deepEqual(endDrag(over), { from: null, over: null });
    assert.equal(endDrag(INITIAL_DRAG), INITIAL_DRAG);
  });

  it("lo store: ciclo completo e nessuna notifica a vuoto", () => {
    const store = createPaneDragStore();
    let calls = 0;
    store.subscribe(() => calls++);
    store.end();
    store.leave("a");
    store.dragOver("b", panes);
    assert.equal(calls, 0);
    store.start("a");
    store.dragOver("b", panes);
    assert.equal(store.getState().from, "a");
    assert.equal(store.getState().over, "b");
    store.end();
    assert.equal(calls, 3);
  });
});
