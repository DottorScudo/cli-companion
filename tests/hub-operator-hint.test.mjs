import { describe, it } from "node:test";
import assert from "node:assert/strict";
import {
  OPERATOR_HINT_STORAGE_KEY,
  OPERATOR_PRESET_ID,
  operatorHintPreset,
  readOperatorHintDismissed,
  writeOperatorHintDismissed,
} from "../src/lib/hubOperatorHint.ts";

const preset = (id, name) => ({
  id,
  label: id,
  description: "Mansione.",
  name,
  system_prompt: "Your job is to help.",
  tools: ["thread_read", "post_message"],
});

const PRESETS = [preset("coordinator", "mario"), preset("reporter", "luca"), preset(OPERATOR_PRESET_ID, "ugo")];

const secretary = (name, presetId = null) => ({
  name,
  image: null,
  system_prompt: "Help.",
  tools: [],
  channels: null,
  preset: presetId,
});

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

const throwingStorage = {
  getItem() {
    throw new Error("denied");
  },
  setItem() {
    throw new Error("quota");
  },
};

describe("operatorHintPreset", () => {
  it("suggerisce il preset dell'Operatore quando non c'è nessun segretario", () => {
    assert.equal(operatorHintPreset([], PRESETS, false)?.id, OPERATOR_PRESET_ID);
  });

  it("lo suggerisce anche accanto ad altri segretari, da preset o da zero", () => {
    const secretaries = [secretary("mario", "coordinator"), secretary("ugo"), secretary("x", "unknown")];
    assert.equal(operatorHintPreset(secretaries, PRESETS, false)?.id, OPERATOR_PRESET_ID);
  });

  it("scompare quando esiste un segretario nato dal preset, anche rinominato", () => {
    assert.equal(operatorHintPreset([secretary("capo-coda", OPERATOR_PRESET_ID)], PRESETS, false), null);
  });

  it("scompare dopo la chiusura", () => {
    assert.equal(operatorHintPreset([], PRESETS, true), null);
  });

  it("non appare se questa versione non conosce il preset o i preset non sono ancora caricati", () => {
    assert.equal(operatorHintPreset([], PRESETS.slice(0, 2), false), null);
    assert.equal(operatorHintPreset([], [], false), null);
  });
});

describe("chiusura del suggerimento in localStorage", () => {
  it("di default non è chiusa", () => {
    assert.equal(readOperatorHintDismissed(memoryStorage()), false);
    assert.equal(readOperatorHintDismissed(null), false);
    assert.equal(readOperatorHintDismissed(undefined), false);
  });

  it("scrive e rilegge la chiusura", () => {
    const storage = memoryStorage();
    writeOperatorHintDismissed(storage);
    assert.equal(storage.data[OPERATOR_HINT_STORAGE_KEY], "dismissed");
    assert.equal(readOperatorHintDismissed(storage), true);
  });

  it("un valore sconosciuto vale non chiusa", () => {
    assert.equal(readOperatorHintDismissed(memoryStorage({ [OPERATOR_HINT_STORAGE_KEY]: "yes" })), false);
  });

  it("ignora gli errori dello storage", () => {
    assert.equal(readOperatorHintDismissed(throwingStorage), false);
    assert.doesNotThrow(() => writeOperatorHintDismissed(throwingStorage));
    assert.doesNotThrow(() => writeOperatorHintDismissed(null));
  });
});
