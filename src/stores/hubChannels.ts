// Canali dell'Agents HUB derivati dai workspace visibili (docs/HUB.MD): Generale
// più uno per workspace. Funzioni pure sullo stato degli archivi, condivise da
// hook (`hooks/useHubChannels.ts`) e azioni (`app/actions/view.ts`).

import { hubChannels } from "../lib/hub.ts";
import type { HubChannel } from "../lib/hub.ts";
import { shallowEqual } from "../lib/store.ts";
import { visibleWorkspaces } from "./workspaces.ts";
import type { WorkspacesState } from "./workspaces.ts";

/** Canali nell'ordine della sidebar; un array nuovo a ogni chiamata (confrontarlo con `sameChannels`). */
export function hubChannelsOf(s: WorkspacesState): HubChannel[] {
  return hubChannels(visibleWorkspaces(s));
}

export function hubChannelIdsOf(s: WorkspacesState): string[] {
  return hubChannelsOf(s).map((c) => c.id);
}

export const sameChannels = (a: HubChannel[], b: HubChannel[]): boolean =>
  a.length === b.length && a.every((c, i) => shallowEqual(c, b[i]));
