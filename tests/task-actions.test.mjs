// La UI legge dal backend cosa si può fare a un task: i task qui hanno la forma
// di `TaskView` e le azioni dichiarate non coincidono per forza con la vecchia
// tabella degli stati, a riprova che il frontend non la ricalcola.
import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { canDo, deleteControl } from "../src/lib/taskActions.ts";

const NO_CASCADE = { dependents: [], running: [] };

const view = (allowed_actions, cascade = NO_CASCADE) => ({ status: "queued", allowed_actions, cascade });

describe("task actions: canDo", () => {
  it("vale solo ciò che il backend elenca", () => {
    const task = view(["to_top", "delete"]);
    assert.equal(canDo(task, "to_top"), true);
    assert.equal(canDo(task, "delete"), true);
    assert.equal(canDo(task, "retry"), false);
    assert.equal(canDo(task, "stop"), false);
  });
  it("lo stato non conta: un task Fatto con azioni dichiarate le ha", () => {
    assert.equal(canDo({ status: "done", allowed_actions: ["retry"] }, "retry"), true);
    assert.equal(canDo({ status: "queued", allowed_actions: [] }, "to_top"), false);
  });
});

describe("task actions: deleteControl", () => {
  it("senza dipendenti: elimina il solo task", () => {
    assert.deepEqual(deleteControl(view(["to_top", "delete"])), { kind: "delete" });
  });
  it("con dipendenti non in corso: elimina a cascata", () => {
    const cascade = { dependents: ["b", "c"], running: [] };
    assert.deepEqual(deleteControl(view(["to_top", "delete_cascade"], cascade)), { kind: "delete_cascade" });
  });
  it("con dipendenti in corso: il pulsante resta ma è bloccato dai task da fermare", () => {
    const cascade = { dependents: ["b", "c"], running: ["c"] };
    assert.deepEqual(deleteControl(view(["retry", "mark_done"], cascade)), { kind: "blocked", running: ["c"] });
  });
  it("un task che non si può eliminare non ha il pulsante", () => {
    assert.equal(deleteControl(view(["stop"])), null);
    assert.equal(deleteControl(view([])), null);
  });
  it("la cascata ha la precedenza se il backend elencasse entrambe", () => {
    const cascade = { dependents: ["b"], running: [] };
    assert.deepEqual(deleteControl(view(["delete", "delete_cascade"], cascade)), { kind: "delete_cascade" });
  });
});
