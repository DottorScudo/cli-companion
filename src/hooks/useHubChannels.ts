import { hubChannelIdsOf, hubChannelsOf, sameChannels } from "../stores/hubChannels";
import { hubStore } from "../stores/hub";
import { workspacesStore } from "../stores/workspaces";
import { resolveChannel, totalWaiting } from "../lib/hub";
import type { HubChannel } from "../lib/hub";
import { shallowEqual } from "../lib/store";
import { useStore, useStores } from "./useStore";

const HUB_WORKSPACES = [hubStore, workspacesStore] as const;

const computeWaitingTotal = () =>
  totalWaiting(hubStore.getState().threads, hubChannelIdsOf(workspacesStore.getState()));

/** Canali dell'Agents HUB (Generale e workspace visibili) e quello mostrato: il selezionato, o il Generale se non esiste più. */
export function useHubChannels(): { channels: HubChannel[]; current: string } {
  const channels = useStore(workspacesStore, hubChannelsOf, sameChannels);
  const channel = useStore(hubStore, (s) => s.channel);
  return { channels, current: resolveChannel(channel, channels) };
}

/** Id dei canali, per chi deve solo sapere quali esistono (non si ridisegna se cambia un nome). */
export function useHubChannelIds(): string[] {
  return useStore(workspacesStore, hubChannelIdsOf, shallowEqual);
}

/** Thread che aspettano l'utente, sommati sui canali visibili (barra dei mondi, barra di stato). */
export function useHubWaitingTotal(): number {
  return useStores(HUB_WORKSPACES, computeWaitingTotal);
}
