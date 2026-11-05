import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  initialValues,
  numberRangeHint,
  secretFields,
  splitBackendError,
  toPayload,
  validateValues,
  valueFields,
} from "../src/lib/connectorForm.ts";

const field = (key, kind, extra = {}) => ({ key, label: key, help: "", required: false, kind, ...extra });
const text = (key, extra = {}) => field(key, { kind: "text", max_len: 10, placeholder: "" }, extra);
const FIELDS = [
  text("name", { required: true }),
  field("token", { kind: "secret" }),
  field("every", { kind: "number", min: 1, max: 60 }),
  field("flag", { kind: "bool", default: true }),
  field("mode", {
    kind: "select",
    options: [
      { value: "a", label: "A" },
      { value: "b", label: "B" },
    ],
  }),
  field("ws", { kind: "workspace" }),
  field("where", { kind: "thread" }),
];

describe("lib/connectorForm: campi", () => {
  it("i segreti non sono valori del form", () => {
    assert.deepEqual(
      valueFields(FIELDS).map((f) => f.key),
      ["name", "every", "flag", "mode", "ws", "where"],
    );
    assert.deepEqual(
      secretFields(FIELDS).map((f) => f.key),
      ["token"],
    );
  });

  it("initialValues: vuoti o predefiniti; da una config salvata prende i valori, mai un segreto", () => {
    assert.deepEqual(initialValues(FIELDS, {}), {
      name: "",
      every: "",
      flag: true,
      mode: "",
      ws: "",
      where: null,
    });
    const saved = initialValues(FIELDS, {
      name: "x",
      token: "non-deve-comparire",
      every: 5,
      flag: false,
      mode: "b",
      ws: "w1",
      where: { channel: "general", thread_id: "thr-1" },
    });
    assert.deepEqual(saved, {
      name: "x",
      every: "5",
      flag: false,
      mode: "b",
      ws: "w1",
      where: { channel: "general", thread_id: "thr-1" },
    });
    assert.equal("token" in saved, false);
  });

  it("initialValues ignora valori salvati del tipo sbagliato", () => {
    const v = initialValues(FIELDS, { name: 3, every: "x", flag: "si", where: { channel: 1 } });
    assert.deepEqual(v, { name: "", every: "", flag: true, mode: "", ws: "", where: null });
  });
});

describe("lib/connectorForm: payload", () => {
  it("toPayload: tipi giusti, spazi tolti, campi opzionali vuoti omessi, bool sempre presente", () => {
    const values = {
      name: "  Mario ",
      every: " 15 ",
      flag: false,
      mode: "",
      ws: "",
      where: { channel: "general", thread_id: "" },
    };
    assert.deepEqual(toPayload(FIELDS, values), { name: "Mario", every: 15, flag: false });
  });

  it("toPayload: un thread scelto resta {channel, thread_id}; il segreto non entra mai", () => {
    const values = { name: "a", token: "segreto", where: { channel: "w1", thread_id: "thr-9" }, mode: "a", ws: "w1" };
    const payload = toPayload(FIELDS, values);
    assert.deepEqual(payload.where, { channel: "w1", thread_id: "thr-9" });
    assert.equal(payload.mode, "a");
    assert.equal(payload.ws, "w1");
    assert.equal("token" in payload, false);
  });
});

describe("lib/connectorForm: validazione locale", () => {
  const ok = { name: "a", every: "10", flag: true, mode: "a", ws: "", where: null };

  it("un form valido non ha errori", () => {
    assert.deepEqual(validateValues(FIELDS, ok), {});
  });

  it("obbligatori vuoti o fatti di spazi", () => {
    assert.deepEqual(validateValues(FIELDS, { ...ok, name: "" }), { name: "Obbligatorio." });
    assert.deepEqual(validateValues(FIELDS, { ...ok, name: "   " }), { name: "Obbligatorio." });
  });

  it("lunghezza massima in caratteri (non in byte)", () => {
    assert.deepEqual(validateValues(FIELDS, { ...ok, name: "àààààààààà" }), {});
    assert.deepEqual(validateValues(FIELDS, { ...ok, name: "ààààààààààà" }), { name: "Al massimo 10 caratteri." });
  });

  it("numeri: interi e nell'intervallo", () => {
    assert.deepEqual(validateValues(FIELDS, { ...ok, every: "0" }), { every: "Da 1 a 60." });
    assert.deepEqual(validateValues(FIELDS, { ...ok, every: "61" }), { every: "Da 1 a 60." });
    assert.deepEqual(validateValues(FIELDS, { ...ok, every: "2.5" }), { every: "Deve essere un numero intero." });
    assert.deepEqual(validateValues(FIELDS, { ...ok, every: "abc" }), { every: "Deve essere un numero intero." });
    assert.deepEqual(validateValues(FIELDS, { ...ok, every: "" }), {});
  });

  it("select: solo le voci dichiarate", () => {
    assert.deepEqual(validateValues(FIELDS, { ...ok, mode: "z" }), { mode: "Scegli una delle voci." });
  });

  it("un obbligatorio di tipo thread senza scelta è mancante; un bool obbligatorio non lo è mai", () => {
    const required = [
      field("where", { kind: "thread" }, { required: true }),
      field("f", { kind: "bool", default: false }, { required: true }),
    ];
    assert.deepEqual(validateValues(required, { where: { channel: "general", thread_id: "" }, f: false }), {
      where: "Obbligatorio.",
    });
  });

  it("numberRangeHint solo per i numeri", () => {
    assert.equal(numberRangeHint(FIELDS[2]), "Da 1 a 60");
    assert.equal(numberRangeHint(FIELDS[0]), null);
  });
});

describe("lib/connectorForm: errori del backend", () => {
  it("riporta l'errore al campo nominato, con l'iniziale maiuscola e il punto", () => {
    assert.deepEqual(splitBackendError("Configurazione non valida: «name»: obbligatorio.", FIELDS), {
      field: "name",
      text: "Obbligatorio.",
    });
    assert.deepEqual(splitBackendError("Filtro non valido: «every»: fuori dall'intervallo 1..=60.", FIELDS), {
      field: "every",
      text: "Fuori dall'intervallo 1..=60.",
    });
  });

  it("un errore senza campo noto resta generale, intero", () => {
    assert.deepEqual(splitBackendError("Configurazione non valida: campo sconosciuto: «zzz».", FIELDS), {
      field: null,
      text: "Configurazione non valida: campo sconosciuto: «zzz».",
    });
    assert.equal(splitBackendError("Configurazione non valida: «zzz»: obbligatorio.", FIELDS).field, null);
    assert.deepEqual(splitBackendError("  L'etichetta deve avere da 1 a 60 caratteri.  ", FIELDS), {
      field: null,
      text: "L'etichetta deve avere da 1 a 60 caratteri.",
    });
  });
});
