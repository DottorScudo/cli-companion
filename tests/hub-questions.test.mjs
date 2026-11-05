import assert from "node:assert/strict";
import { describe, it } from "node:test";

import {
  QUESTION_ANSWERED_LABEL,
  QUESTION_CLOSED_LABEL,
  QUESTION_FROM_PLANNER,
  QUESTION_FROM_WORKER,
  QUESTION_WAITING_LABEL,
  THREAD_SEARCH_LIMIT,
  deliveryFailures,
  questionHeading,
  questionStateLabel,
  questionStates,
  threadAsksTask,
  threadsToSearch,
} from "../src/lib/hubQuestions.ts";

let counter = 0;
const message = (id, extra = {}) => ({
  id,
  author: { type: "system" },
  kind: "event",
  body: id,
  reply_to: null,
  event: null,
  created_at: ++counter,
  ...extra,
});
const event = (kind, taskId, extra = {}) => ({
  kind,
  workspace: "ws",
  task_id: taskId,
  proposal_id: null,
  status: null,
  summary: null,
  verdict: null,
  note: null,
  question: null,
  ...extra,
});
const hubQuestion = (id) => ({ id, text: "Quale?", options: ["A", "B"] });
const ask = (id, taskId = "t1", questionId = `q-${id}`) =>
  message(id, {
    body: "Domanda del worker: Titolo",
    event: event("task_question", taskId, { question: hubQuestion(questionId) }),
  });
const answeredEvent = (id, taskId, questionId) =>
  message(id, { event: event("task_question_answered", taskId, { question: hubQuestion(questionId) }) });
const userReply = (id, to, body = "A") => message(id, { author: { type: "user" }, kind: "text", body, reply_to: to });
const outcome = (id, to, body) => message(id, { kind: "text", body, reply_to: to });
const SENT = "Risposta inviata al worker.";
const FAILED = "Risposta non inviata: il worker sta ancora lavorando. Se serve, apri il terminale del worker.";

describe("questionStates", () => {
  it("una domanda senza altro dopo aspetta l'utente", () => {
    assert.deepEqual(questionStates([ask("m1")]), { m1: { kind: "pending" } });
  });
  it("la consegna riuscita la chiude subito, con il testo inviato", () => {
    const states = questionStates([ask("m1"), userReply("u1", "m1", "Seconda opzione"), outcome("s1", "u1", SENT)]);
    assert.deepEqual(states.m1, { kind: "answered", answer: "Seconda opzione" });
  });
  it("la consegna fallita la lascia in attesa", () => {
    const states = questionStates([ask("m1"), userReply("u1", "m1"), outcome("s1", "u1", FAILED)]);
    assert.deepEqual(states.m1, { kind: "pending" });
  });
  it("dopo un tentativo fallito e uno riuscito vale il riuscito", () => {
    const states = questionStates([
      ask("m1"),
      userReply("u1", "m1", "A"),
      outcome("s1", "u1", FAILED),
      userReply("u2", "m1", "B"),
      outcome("s2", "u2", SENT),
    ]);
    assert.deepEqual(states.m1, { kind: "answered", answer: "B" });
  });
  it("un messaggio dell'utente senza esito non chiude ancora la domanda", () => {
    assert.deepEqual(questionStates([ask("m1"), userReply("u1", "m1")]).m1, { kind: "pending" });
  });
  it("la risposta vista dalla board (terminale o segretario) chiude senza testo", () => {
    const states = questionStates([ask("m1"), answeredEvent("e1", "t1", "q-m1")]);
    assert.deepEqual(states.m1, { kind: "answered", answer: null });
  });
  it("il task che va avanti senza risposta chiude la domanda", () => {
    for (const kind of ["task_done", "task_failed", "task_deleted", "task_started"]) {
      const states = questionStates([ask("m1"), message("e1", { event: event(kind, "t1") })]);
      assert.deepEqual(states.m1, { kind: "closed" }, kind);
    }
  });
  it("una domanda nuova chiude la vecchia e aspetta lei", () => {
    const states = questionStates([
      ask("m1", "t1", "q-1-1"),
      answeredEvent("e1", "t1", "q-1-1"),
      ask("m2", "t1", "q-1-2"),
    ]);
    assert.deepEqual(states, { m1: { kind: "answered", answer: null }, m2: { kind: "pending" } });
  });
  it("gli eventi di altri task o altri workspace non toccano la domanda", () => {
    const states = questionStates([
      ask("m1"),
      message("e1", { event: event("task_done", "t2") }),
      message("e2", { event: event("task_done", "t1", { workspace: "altro" }) }),
    ]);
    assert.deepEqual(states.m1, { kind: "pending" });
  });
  it("ignora messaggi senza domanda e domande senza task", () => {
    const noQuestion = message("m0", { event: event("task_question", "t1") });
    const noTask = message("m9", { event: event("task_question", null, { question: hubQuestion("q") }) });
    assert.deepEqual(questionStates([message("t"), noQuestion, noTask]), {});
  });
});

describe("deliveryFailures", () => {
  it("lega l'esito fallito all'evento della domanda", () => {
    const failures = deliveryFailures([ask("m1"), userReply("u1", "m1"), outcome("s1", "u1", FAILED)]);
    assert.deepEqual(Object.keys(failures), ["s1"]);
    assert.equal(failures.s1.task_id, "t1");
  });
  it("comprende anche «questa domanda ha già una risposta»", () => {
    const rejected = "Risposta non inviata: questa domanda ha già una risposta o il suo task non è più in attesa.";
    const failures = deliveryFailures([ask("m1"), userReply("u1", "m1"), outcome("s1", "u1", rejected)]);
    assert.deepEqual(Object.keys(failures), ["s1"]);
  });
  it("non tocca gli esiti riusciti, i messaggi dell'utente e le risposte ad altro", () => {
    const failures = deliveryFailures([
      ask("m1"),
      userReply("u1", "m1"),
      outcome("s1", "u1", SENT),
      message("a1", { kind: "text" }),
      userReply("u2", "a1"),
      outcome("s2", "u2", FAILED),
      message("s3", { author: { type: "user" }, body: FAILED, reply_to: "u1" }),
      outcome("s4", "mancante", FAILED),
    ]);
    assert.deepEqual(failures, {});
  });
});

describe("testi della scheda", () => {
  it("titolo: pianificatore dal task `plan`, altrimenti dal testo dell'evento", () => {
    assert.equal(questionHeading("Domanda del worker: X", { kind: "plan" }), QUESTION_FROM_PLANNER);
    assert.equal(questionHeading("Domanda del pianificatore: X", { kind: "work" }), QUESTION_FROM_WORKER);
    assert.equal(questionHeading("Domanda del pianificatore: X", null), QUESTION_FROM_PLANNER);
    assert.equal(questionHeading("Domanda del worker: X", null), QUESTION_FROM_WORKER);
  });
  it("etichetta dello stato", () => {
    assert.equal(questionStateLabel({ kind: "pending" }), QUESTION_WAITING_LABEL);
    assert.equal(questionStateLabel({ kind: "answered", answer: null }), QUESTION_ANSWERED_LABEL);
    assert.equal(questionStateLabel({ kind: "closed" }), QUESTION_CLOSED_LABEL);
    assert.equal(QUESTION_WAITING_LABEL, "Aspetta te");
    assert.equal(QUESTION_ANSWERED_LABEL, "Risposta inviata");
  });
});

describe("ricerca del thread di un task", () => {
  it("threadAsksTask: serve una domanda dello stesso task nello stesso workspace", () => {
    const thread = { messages: [ask("m1", "t1")] };
    assert.equal(threadAsksTask(thread, "ws", "t1"), true);
    assert.equal(threadAsksTask(thread, "ws", "t2"), false);
    assert.equal(threadAsksTask(thread, "altro", "t1"), false);
    assert.equal(threadAsksTask({ messages: [message("e", { event: event("task_done", "t1") })] }, "ws", "t1"), false);
  });
  it("threadsToSearch: prima chi aspetta l'utente, poi i più recenti; chiusi esclusi", () => {
    const t = (id, extra) => ({ id, state: "open", waiting_user: false, updated_at: 1, ...extra });
    const ids = threadsToSearch([
      t("vecchio", { updated_at: 1 }),
      t("chiuso", { state: "closed", waiting_user: true, updated_at: 9 }),
      t("recente", { updated_at: 5 }),
      t("attende", { waiting_user: true, updated_at: 2 }),
    ]);
    assert.deepEqual(ids, ["attende", "recente", "vecchio"]);
  });
  it("threadsToSearch: ne legge al massimo THREAD_SEARCH_LIMIT", () => {
    const many = Array.from({ length: THREAD_SEARCH_LIMIT + 5 }, (_, i) => ({
      id: `t${i}`,
      state: "open",
      waiting_user: false,
      updated_at: i,
    }));
    assert.equal(threadsToSearch(many).length, THREAD_SEARCH_LIMIT);
  });
});
