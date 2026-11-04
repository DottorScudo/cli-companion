import { useEffect, useMemo } from "react";
import { loadAllChannels, loadThread, selectChannel } from "../../app/actions/hub";
import { useStore } from "../../hooks/useStore";
import { GENERAL_CHANNEL, hubChannels, resolveChannel } from "../../lib/hub";
import { shallowEqual } from "../../lib/store";
import { hubStore } from "../../stores/hub";
import { visibleWorkspaces, workspacesStore } from "../../stores/workspaces";
import type { WorkspacesState } from "../../stores/workspaces";
import HubChannelList from "./HubChannelList";
import HubConversation from "./HubConversation";
import HubThreadList from "./HubThreadList";

interface WorkspaceName {
  id: string;
  name: string;
}

const workspaceNames = (s: WorkspacesState): WorkspaceName[] =>
  visibleWorkspaces(s).map((w) => ({ id: w.id, name: w.name }));
const sameWorkspaces = (a: WorkspaceName[], b: WorkspaceName[]) =>
  a.length === b.length && a.every((w, i) => shallowEqual(w, b[i]));

/**
 * Vista a tutta larghezza a tre colonne: canali, thread del canale e conversazione. All'apertura
 * legge l'elenco di ogni canale (per i segnalini) e rilegge il thread rimasto
 * aperto; poi segue `hub-update`, senza interrogazioni periodiche.
 */
export default function HubView() {
  const workspaces = useStore(workspacesStore, workspaceNames, sameWorkspaces);
  const channel = useStore(hubStore, (s) => s.channel);
  const channels = useMemo(() => hubChannels(workspaces), [workspaces]);
  const channelIds = useMemo(() => channels.map((c) => c.id).join("\n"), [channels]);
  const current = resolveChannel(channel, channels);

  useEffect(() => {
    loadAllChannels(channelIds.split("\n"));
  }, [channelIds]);

  useEffect(() => {
    const { channel: openChannel, threadId } = hubStore.getState();
    if (threadId !== null) loadThread(openChannel, threadId);
  }, []);

  // Il workspace del canale selezionato non c'è più (rimosso o archiviato).
  useEffect(() => {
    if (current !== channel) selectChannel(GENERAL_CHANNEL);
  }, [current, channel]);

  return (
    <div className="flex h-full min-h-0">
      <HubChannelList channels={channels} selected={current} />
      <HubThreadList channel={current} channelName={channels.find((c) => c.id === current)?.name ?? ""} />
      <HubConversation channel={current} />
    </div>
  );
}
