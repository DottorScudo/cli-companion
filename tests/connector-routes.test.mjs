import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  buildRouteInput,
  destinationText,
  existingTargetChannels,
  hasProblems,
  newRouteForm,
  routeFormOf,
  routeInputWithEnabled,
  routesOfInstance,
  routesToThread,
  suggestRouteLabel,
  threadTitle,
  validateRouteForm,
  wakeForChannel,
  wakeLabel,
  wakeOptions,
} from "../src/lib/connectorRoutes.ts";

const FILTER = [
  { key: "query", label: "Query", help: "", required: true, kind: { kind: "text", max_len: 200, placeholder: "" } },
  { key: "unread", label: "Non letti", help: "", required: false, kind: { kind: "bool", default: false } },
];
const secretary = (name, channels = { mode: "all" }, prompt = "Fai questo") => ({
  name,
  system_prompt: prompt,
  channels,
});
const SECRETARIES = [
  secretary("anna"),
  secretary("bruno", { mode: "only", channels: ["ws-1"] }),
  secretary("vuoto", { mode: "all" }, "  "),
];
const route = (id, extra = {}) => ({
  id,
  instance: "con-1-1",
  label: id,
  filter: { query: "from:mario" },
  channel: "ws-1",
  thread: { mode: "existing", thread_id: "thr-1" },
  wake: "none",
  enabled: true,
  error: null,
  created_at: 1,
  updated_at: 1,
  ...extra,
});
const CHANNELS = [
  { id: "general", name: "Generale" },
  { id: "ws-1", name: "Progetto" },
];
const summary = (id, title) => ({ id, title });

describe("lib/connectorRoutes: form", () => {
  it("newRouteForm parte da un thread esistente nel Generale, senza risveglio, abilitata", () => {
    assert.deepEqual(newRouteForm(FILTER), {
      label: "",
      filter: { query: "", unread: false },
      channel: "general",
      mode: "existing",
      threadId: "",
      wake: "none",
      enabled: true,
    });
    assert.equal(newRouteForm(FILTER, "ws-1").channel, "ws-1");
  });

  it("routeFormOf riporta una regola salvata nel form", () => {
    const form = routeFormOf(route("rte-1", { wake: "owner", enabled: false }), FILTER);
    assert.deepEqual(form, {
      label: "rte-1",
      filter: { query: "from:mario", unread: false },
      channel: "ws-1",
      mode: "existing",
      threadId: "thr-1",
      wake: "owner",
      enabled: false,
    });
    const perItem = routeFormOf(route("rte-2", { thread: { mode: "new_per_item" } }), FILTER);
    assert.equal(perItem.mode, "new_per_item");
    assert.equal(perItem.threadId, "");
  });

  it("buildRouteInput: thread esistente con il suo id, altre modalità senza", () => {
    const form = { ...newRouteForm(FILTER, "ws-1"), label: " Mario ", threadId: "thr-7" };
    form.filter = { query: " from:mario ", unread: true };
    assert.deepEqual(buildRouteInput(form, FILTER, "Posta"), {
      label: "Mario",
      filter: { query: "from:mario", unread: true },
      channel: "ws-1",
      thread: { mode: "existing", thread_id: "thr-7" },
      wake: "none",
      enabled: true,
    });
    const perConv = buildRouteInput({ ...form, mode: "per_conversation", threadId: "thr-7" }, FILTER, "Posta");
    assert.deepEqual(perConv.thread, { mode: "per_conversation" });
  });

  it("l'etichetta vuota diventa il primo testo del filtro, altrimenti il nome del connettore, al massimo 60 caratteri", () => {
    assert.equal(suggestRouteLabel(FILTER, { query: " from:mario ", unread: false }, "Posta"), "from:mario");
    assert.equal(suggestRouteLabel(FILTER, { query: "", unread: false }, "Posta"), "Posta");
    assert.equal(suggestRouteLabel(FILTER, { query: "x".repeat(100) }, "Posta").length, 60);
    const form = { ...newRouteForm(FILTER), threadId: "thr-1", filter: { query: "q", unread: false } };
    assert.equal(buildRouteInput(form, FILTER, "Posta").label, "q");
  });

  it("validateRouteForm: filtro obbligatorio e thread mancante", () => {
    const problems = validateRouteForm(newRouteForm(FILTER), FILTER);
    assert.deepEqual(problems.filter, { query: "Obbligatorio." });
    assert.equal(problems.thread, "Scegli il thread di destinazione.");
    assert.equal(hasProblems(problems), true);
    const fine = { ...newRouteForm(FILTER), threadId: "thr-1", filter: { query: "q", unread: false } };
    assert.equal(hasProblems(validateRouteForm(fine, FILTER)), false);
    const perItem = { ...fine, mode: "new_per_item", threadId: "" };
    assert.equal(validateRouteForm(perItem, FILTER).thread, null);
  });

  it("routeInputWithEnabled rifà la regola cambiando solo l'abilitazione", () => {
    const r = route("rte-1", { wake: "secretary:anna" });
    assert.deepEqual(routeInputWithEnabled(r, false), {
      label: "rte-1",
      filter: r.filter,
      channel: "ws-1",
      thread: r.thread,
      wake: "secretary:anna",
      enabled: false,
    });
  });
});

describe("lib/connectorRoutes: risveglio", () => {
  it("nessuno, il titolare e i segretari configurati e ammessi nel canale", () => {
    assert.deepEqual(
      wakeOptions(SECRETARIES, "ws-1", true).map((o) => o.value),
      ["none", "owner", "secretary:anna", "secretary:bruno"],
    );
    assert.deepEqual(
      wakeOptions(SECRETARIES, "general", true).map((o) => o.value),
      ["none", "owner", "secretary:anna"],
    );
  });

  it("un connettore che non può svegliare offre solo «nessuno»", () => {
    assert.deepEqual(
      wakeOptions(SECRETARIES, "ws-1", false).map((o) => o.value),
      ["none"],
    );
  });

  it("un valore salvato non più fra le scelte resta visibile", () => {
    const values = wakeOptions(SECRETARIES, "general", true, "secretary:bruno").map((o) => o.value);
    assert.deepEqual(values, ["none", "owner", "secretary:anna", "secretary:bruno"]);
  });

  it("cambiando canale un segretario non più ammesso torna a «nessuno»", () => {
    assert.equal(wakeForChannel("secretary:bruno", SECRETARIES, "general", true), "none");
    assert.equal(wakeForChannel("secretary:bruno", SECRETARIES, "ws-1", true), "secretary:bruno");
    assert.equal(wakeForChannel("owner", SECRETARIES, "general", true), "owner");
    assert.equal(wakeForChannel("owner", SECRETARIES, "general", false), "none");
  });

  it("wakeLabel", () => {
    assert.equal(wakeLabel("none"), "Nessun risveglio");
    assert.equal(wakeLabel("owner"), "Sveglia il titolare");
    assert.equal(wakeLabel("secretary:anna"), "Sveglia @anna");
  });
});

describe("lib/connectorRoutes: elenchi ed etichette", () => {
  const routes = [
    route("a"),
    route("b", { instance: "con-2-1" }),
    route("c", { thread: { mode: "per_conversation" } }),
    route("d", { thread: { mode: "existing", thread_id: "thr-2" } }),
    route("e", { channel: "general" }),
  ];

  it("routesOfInstance e routesToThread (solo destinazioni esistenti nello stesso canale)", () => {
    assert.deepEqual(
      routesOfInstance(routes, "con-1-1").map((r) => r.id),
      ["a", "c", "d", "e"],
    );
    assert.deepEqual(
      routesToThread(routes, "ws-1", "thr-1").map((r) => r.id),
      ["a", "b"],
    );
    assert.deepEqual(
      routesToThread(routes, "general", "thr-1").map((r) => r.id),
      ["e"],
    );
    assert.deepEqual(routesToThread(routes, "ws-1", "thr-9"), []);
  });

  it("existingTargetChannels: i canali, una volta sola", () => {
    assert.deepEqual(existingTargetChannels(routes), ["ws-1", "general"]);
    assert.deepEqual(existingTargetChannels([]), []);
  });

  it("threadTitle e destinationText", () => {
    const threads = { "ws-1": [summary("thr-1", "Posta di Mario")] };
    assert.equal(threadTitle(threads, "ws-1", "thr-1"), "Posta di Mario");
    assert.equal(threadTitle(threads, "ws-1", "thr-9"), null);
    assert.equal(threadTitle(threads, "general", "thr-1"), null);
    assert.equal(destinationText(route("a"), CHANNELS, threads), "Progetto · Posta di Mario");
    assert.equal(
      destinationText(route("a", { thread: { mode: "existing", thread_id: "thr-9" } }), CHANNELS, threads),
      "Progetto · thread non più disponibile",
    );
    assert.equal(destinationText(route("e", { channel: "general" }), CHANNELS, threads), "Generale · thread");
    assert.equal(
      destinationText(route("c", { thread: { mode: "new_per_item" } }), CHANNELS, threads),
      "Progetto · un nuovo thread per elemento",
    );
    assert.equal(
      destinationText(route("c", { thread: { mode: "per_conversation" } }), CHANNELS, threads),
      "Progetto · un thread per conversazione",
    );
  });
});
