import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  PROPOSAL_STATUS_LABEL,
  cardStatus,
  planTaskOf,
  proposalCount,
  proposalSpecs,
  proposalStatuses,
  proposalTitle,
} from "../src/lib/hubProposals.ts";

const message = (kind, proposalId, extra = {}) => ({
  event: { kind, workspace: "ws", task_id: null, proposal_id: proposalId, ...extra },
});
const spec = (id) => ({ id, title: `Task ${id}`, task: "spec", depends_on: [], model: null, kind: "work" });

describe("proposalStatuses", () => {
  it("vale l'ultimo evento di ogni proposta", () => {
    const statuses = proposalStatuses([
      message("proposal_created", "p1"),
      message("proposal_created", "p2"),
      message("proposal_accepted", "p1"),
      message("proposal_created", "p3"),
      message("proposal_rejected", "p3"),
    ]);
    assert.deepEqual(statuses, { p1: "accepted", p2: "pending", p3: "rejected" });
  });
  it("ignora messaggi di testo, eventi dei task e eventi senza proposta", () => {
    assert.deepEqual(
      proposalStatuses([
        { event: null },
        message("task_done", null, { task_id: "t" }),
        message("proposal_created", null),
        message("evento_nuovo", "p1"),
      ]),
      {},
    );
  });
  it("le etichette coprono ogni stato", () => {
    assert.deepEqual(Object.keys(PROPOSAL_STATUS_LABEL).sort(), ["accepted", "pending", "rejected"]);
  });
});

describe("cardStatus", () => {
  const statuses = { p1: "pending", p2: "accepted" };
  it("solo «proposta pronta» è una scheda", () => {
    assert.equal(cardStatus(message("proposal_created", "p1"), statuses), "pending");
    assert.equal(cardStatus(message("proposal_accepted", "p2"), statuses), undefined);
    assert.equal(cardStatus({ event: null }, statuses), undefined);
    assert.equal(cardStatus(message("proposal_created", null), statuses), undefined);
    assert.equal(cardStatus(message("proposal_created", "sconosciuta"), statuses), undefined);
  });
});

describe("proposalSpecs", () => {
  const boards = { ws: { proposals: [{ id: "p1", tasks: [spec("a"), spec("b")] }] } };
  it("i task della proposta pendente nella board", () => {
    assert.deepEqual(
      proposalSpecs(boards, "ws", "p1")?.map((s) => s.id),
      ["a", "b"],
    );
  });
  it("null se la proposta non c'è più, la board non è letta o manca il workspace", () => {
    assert.equal(proposalSpecs(boards, "ws", "p9"), null);
    assert.equal(proposalSpecs(boards, "altro", "p1"), null);
    assert.equal(proposalSpecs(boards, null, "p1"), null);
  });
});

describe("proposalCount e proposalTitle", () => {
  it("il numero viene dall'elenco della board, altrimenti dal testo del messaggio", () => {
    assert.equal(proposalCount([spec("a"), spec("b")], "Nuova proposta: 5 task"), 2);
    assert.equal(proposalCount(null, "Nuova proposta: 5 task\n- A"), 5);
    assert.equal(proposalCount(null, "altro testo"), null);
  });
  it("titolo della scheda", () => {
    assert.equal(proposalTitle(3), "Proposta pronta · 3 task");
    assert.equal(proposalTitle(1), "Proposta pronta · 1 task");
    assert.equal(proposalTitle(null), "Proposta pronta");
  });
});

describe("planTaskOf", () => {
  const boards = {
    ws: {
      tasks: [
        { id: "plan-x", kind: "plan" },
        { id: "work-y", kind: "work" },
      ],
    },
  };
  it("il task plan dell'evento", () => {
    assert.equal(planTaskOf(boards, { workspace: "ws", task_id: "plan-x" })?.id, "plan-x");
  });
  it("null per task che non sono piani, mancanti o eventi senza task o workspace", () => {
    assert.equal(planTaskOf(boards, { workspace: "ws", task_id: "work-y" }), null);
    assert.equal(planTaskOf(boards, { workspace: "ws", task_id: "ghost" }), null);
    assert.equal(planTaskOf(boards, { workspace: "altro", task_id: "plan-x" }), null);
    assert.equal(planTaskOf(boards, { workspace: "ws", task_id: null }), null);
    assert.equal(planTaskOf(boards, { workspace: null, task_id: "plan-x" }), null);
  });
});
