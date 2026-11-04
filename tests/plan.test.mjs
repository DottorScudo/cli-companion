import assert from "node:assert/strict";
import { describe, it } from "node:test";

import { PLAN_LABEL, isPlan, planQuestions } from "../src/lib/plan.ts";

const plan = (over = {}) => ({
  id: "p",
  kind: "plan",
  status: "done",
  report: { status: "completed", summary: "Questions for the user:\n1. Quale database?\n2. Serve il login?" },
  ...over,
});

describe("isPlan", () => {
  it("riconosce solo kind plan (assente = work)", () => {
    assert.equal(PLAN_LABEL, "Pianificazione");
    assert.equal(isPlan({ kind: "plan" }), true);
    assert.equal(isPlan({ kind: "review" }), false);
    assert.equal(isPlan({ kind: "work" }), false);
    assert.equal(isPlan({}), false);
  });
});

describe("planQuestions", () => {
  it("le domande dopo il marcatore, senza il marcatore", () => {
    assert.equal(planQuestions(plan()), "1. Quale database?\n2. Serve il login?");
  });
  it("ignora maiuscole, due punti e spazi attorno", () => {
    const t = plan({ report: { status: "completed", summary: "  QUESTIONS FOR THE USER\n- a?  " } });
    assert.equal(planQuestions(t), "- a?");
  });
  it("niente per un piano che ha proposto, non concluso o senza report", () => {
    assert.equal(planQuestions(plan({ report: { status: "completed", summary: "Proposed 3 tasks." } })), null);
    assert.equal(planQuestions(plan({ status: "running" })), null);
    assert.equal(planQuestions(plan({ status: "failed" })), null);
    assert.equal(planQuestions(plan({ report: null })), null);
  });
  it("niente se il marcatore non ha domande dopo", () => {
    assert.equal(planQuestions(plan({ report: { status: "completed", summary: "Questions for the user:" } })), null);
  });
  it("niente per i task che non sono piani", () => {
    assert.equal(planQuestions(plan({ kind: "work" })), null);
    assert.equal(planQuestions(plan({ kind: "review" })), null);
  });
});
