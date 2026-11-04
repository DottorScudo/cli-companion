import { describe, it } from "node:test";
import assert from "node:assert/strict";
import { hubChannelIdsOf, hubChannelsOf, sameChannels } from "../src/stores/hubChannels.ts";
import { INITIAL_WORKSPACES, loaded } from "../src/stores/workspaces.ts";

const ws = (id, extra = {}) => ({ id, name: `Workspace ${id}`, ...extra });
const state = loaded(INITIAL_WORKSPACES, [ws("a"), ws("b", { archived: true }), ws("c")]);

describe("hubChannels: canali dai workspace", () => {
  it("Generale in cima, poi i soli workspace visibili nell'ordine della sidebar", () => {
    assert.deepEqual(hubChannelIdsOf(state), ["general", "a", "c"]);
    assert.deepEqual(hubChannelsOf(state)[1], { id: "a", name: "Workspace a" });
  });

  it("sameChannels: stessi id e nomi, anche in array diversi", () => {
    assert.equal(sameChannels(hubChannelsOf(state), hubChannelsOf(state)), true);
    const renamed = loaded(INITIAL_WORKSPACES, [ws("a", { name: "Altro" }), ws("c")]);
    assert.equal(sameChannels(hubChannelsOf(state), hubChannelsOf(renamed)), false);
  });
});
