import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { draftOfPreset, presetNameError, presetOf } from "../src/lib/hubPresets.ts";

const preset = (id, name, extra = {}) => ({
  id,
  label: id,
  description: "Mansione.",
  name,
  system_prompt: "Your job is to help.",
  tools: ["thread_read", "post_message"],
  ...extra,
});

const PRESETS = [preset("coordinator", "mario"), preset("analyst", "giulia"), preset("reporter", "luca")];

const secretary = (name, extra = {}) => ({
  name,
  image: null,
  system_prompt: "You help.",
  tools: [],
  channels: { mode: "all" },
  preset: null,
  ...extra,
});

describe("draftOfPreset", () => {
  it("prefills name, prompt and tools, in every channel", () => {
    assert.deepEqual(draftOfPreset(PRESETS[1]), {
      name: "giulia",
      systemPrompt: "Your job is to help.",
      tools: ["thread_read", "post_message"],
      channels: null,
    });
  });
});

describe("presetOf", () => {
  it("finds the preset a secretary comes from", () => {
    assert.equal(presetOf(secretary("capo", { preset: "coordinator" }), PRESETS), PRESETS[0]);
  });

  it("is null for a secretary made from scratch or from an unknown preset", () => {
    assert.equal(presetOf(secretary("mario"), PRESETS), null);
    assert.equal(presetOf(secretary("mario", { preset: "from-the-future" }), PRESETS), null);
    assert.equal(presetOf(secretary("mario", { preset: "coordinator" }), []), null);
  });
});

describe("presetNameError", () => {
  it("is null when the suggested name is free", () => {
    assert.equal(presetNameError(PRESETS[0], ["giulia"]), null);
  });

  it("reports a name already taken, whatever its case", () => {
    assert.match(presetNameError(PRESETS[0], ["Mario"]), /già/);
  });
});
