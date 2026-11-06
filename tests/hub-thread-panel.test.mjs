import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  activeThreadPanelTasks,
  sameThreadPanelTasks,
  threadPanelSessions,
  threadPanelTasks,
} from "../src/lib/hubThreadPanel.ts";
import {
  DEFAULT_THREAD_PANEL_SHOWN,
  THREAD_PANEL_STORAGE_KEY,
  parseThreadPanelShown,
  readThreadPanelShown,
  writeThreadPanelShown,
} from "../src/lib/hubThreadPanelPref.ts";
import { createHubStore, threadPanelShownSet } from "../src/stores/hub.ts";

const task = (id, session = null) => ({ id, title: id, session });
const boards = {
  ws1: { tasks: [task("a", "tab-a"), task("b", "tab-b"), task("c")] },
  ws2: { tasks: [task("d", "tab-d")] },
};

function memoryStorage(initial = {}) {
  const data = { ...initial };
  return {
    data,
    getItem: (k) => data[k] ?? null,
    setItem: (k, v) => {
      data[k] = v;
    },
  };
}

describe("threadPanelTasks", () => {
  it("segue l'ordine di task_ids e salta i task non più nella board", () => {
    const result = threadPanelTasks({ channel: "ws1", task_ids: ["b", "gone", "a"] }, boards);
    assert.deepEqual(
      result.map((r) => [r.task.id, r.workspace]),
      [
        ["b", "ws1"],
        ["a", "ws1"],
      ],
    );
  });

  it("in un canale di workspace cerca solo nella sua board", () => {
    assert.deepEqual(threadPanelTasks({ channel: "ws1", task_ids: ["d"] }, boards), []);
  });

  it("nel Generale cerca in tutte le board e usa il workspace trovato", () => {
    const result = threadPanelTasks({ channel: "general", task_ids: ["d", "a"] }, boards);
    assert.deepEqual(
      result.map((r) => [r.task.id, r.workspace]),
      [
        ["d", "ws2"],
        ["a", "ws1"],
      ],
    );
  });

  it("un thread senza task o una board assente danno una lista vuota", () => {
    assert.deepEqual(threadPanelTasks({ channel: "ws1", task_ids: [] }, boards), []);
    assert.deepEqual(threadPanelTasks({ channel: "nope", task_ids: ["a"] }, boards), []);
  });
});

describe("threadPanelSessions", () => {
  const tasks = threadPanelTasks({ channel: "general", task_ids: ["a", "b", "c", "d"] }, boards);

  it("tiene solo i tab ancora aperti, nell'ordine dei task", () => {
    assert.deepEqual(
      threadPanelSessions(tasks, (key) => key !== "tab-b"),
      ["tab-a", "tab-d"],
    );
  });

  it("un task senza session non dà terminali e nessun tab aperto dà lista vuota", () => {
    assert.deepEqual(
      threadPanelSessions(tasks, () => false),
      [],
    );
  });

  it("non ripete una session condivisa", () => {
    const shared = [
      { task: task("x", "tab-x"), workspace: "ws1" },
      { task: task("y", "tab-x"), workspace: "ws1" },
    ];
    assert.deepEqual(
      threadPanelSessions(shared, () => true),
      ["tab-x"],
    );
  });
});

describe("activeThreadPanelTasks", () => {
  const entry = (id, status, extra = {}) => ({ task: { ...task(id), status, ...extra }, workspace: "ws1" });

  it("tiene i task in corso (anche con attenzione) e in coda, nell'ordine ricevuto", () => {
    const tasks = [
      entry("r", "running"),
      entry("d", "done"),
      entry("q", "queued"),
      entry("f", "failed"),
      entry("att", "running", { attention: true }),
    ];
    assert.deepEqual(
      activeThreadPanelTasks(tasks).map((r) => r.task.id),
      ["r", "q", "att"],
    );
  });

  it("restituisce gli stessi oggetti ricevuti", () => {
    const tasks = [entry("q", "queued")];
    assert.equal(activeThreadPanelTasks(tasks)[0], tasks[0]);
  });

  it("dà una lista vuota se restano solo task conclusi o falliti, o se non ce ne sono", () => {
    assert.deepEqual(activeThreadPanelTasks([entry("d", "done"), entry("f", "failed")]), []);
    assert.deepEqual(activeThreadPanelTasks([]), []);
  });
});

describe("sameThreadPanelTasks", () => {
  const first = threadPanelTasks({ channel: "ws1", task_ids: ["a", "b"] }, boards);

  it("è vero per gli stessi oggetti e falso se cambia un task, il workspace o la lunghezza", () => {
    assert.equal(sameThreadPanelTasks(first, threadPanelTasks({ channel: "ws1", task_ids: ["a", "b"] }, boards)), true);
    assert.equal(sameThreadPanelTasks(first, [first[0], { ...first[1], task: { ...first[1].task } }]), false);
    assert.equal(sameThreadPanelTasks(first, [first[0], { ...first[1], workspace: "ws2" }]), false);
    assert.equal(sameThreadPanelTasks(first, [first[0]]), false);
  });
});

describe("preferenza del pannello", () => {
  it("di default è visibile e un valore ignoto vale il default", () => {
    assert.equal(DEFAULT_THREAD_PANEL_SHOWN, true);
    assert.equal(parseThreadPanelShown("boh"), true);
    assert.equal(parseThreadPanelShown(null), true);
    assert.equal(parseThreadPanelShown("hidden"), false);
  });

  it("scrive e rilegge; storage assente o che lancia non rompe", () => {
    const storage = memoryStorage();
    writeThreadPanelShown(storage, false);
    assert.equal(storage.data[THREAD_PANEL_STORAGE_KEY], "hidden");
    assert.equal(readThreadPanelShown(storage), false);
    assert.equal(readThreadPanelShown(null), true);
    const broken = {
      getItem: () => {
        throw new Error("no");
      },
      setItem: () => {
        throw new Error("no");
      },
    };
    assert.equal(readThreadPanelShown(broken), true);
    assert.doesNotThrow(() => writeThreadPanelShown(broken, false));
  });
});

describe("threadPanelShown nello store", () => {
  it("la transizione non cambia lo stato se il valore è lo stesso", () => {
    const state = createHubStore(null).getState();
    assert.equal(threadPanelShownSet(state, true), state);
    assert.equal(threadPanelShownSet(state, false).threadPanelShown, false);
  });

  it("si salva a ogni cambio e si ripristina al riavvio", () => {
    const storage = memoryStorage();
    createHubStore(storage).threadPanelShownSet(false);
    assert.equal(storage.data[THREAD_PANEL_STORAGE_KEY], "hidden");
    assert.equal(createHubStore(storage).getState().threadPanelShown, false);
  });
});
