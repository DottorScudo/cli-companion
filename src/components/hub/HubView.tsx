import { useEffect, useMemo } from "react";
import { ArrowLeft, MessagesSquare } from "lucide-react";
import { closeHub } from "../../app/actions/view";
import { loadAllChannels, loadThread, selectChannel } from "../../app/actions/hub";
import { useStore } from "../../hooks/useStore";
import { GENERAL_CHANNEL, hubChannels, resolveChannel } from "../../lib/hub";
import { shallowEqual } from "../../lib/store";
import { hubStore } from "../../stores/hub";
import { visibleWorkspaces, workspacesStore } from "../../stores/workspaces";
import type { WorkspacesState } from "../../stores/workspaces";
import { Button } from "../ui/button";
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
 * Vista a tre colonne: canali, thread del canale e conversazione. All'apertura
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
    <div className="flex h-full min-h-0 flex-col">
      <header className="flex flex-shrink-0 items-center gap-3 border-b bg-base px-4 py-2.5">
        <Button
          type="button"
          variant="outline"
          size="sm"
          onClick={closeHub}
          className="h-8 gap-1.5 bg-transparent text-[13px] text-text-secondary"
        >
          <ArrowLeft className="!size-3.5" /> Torna ai terminali
        </Button>
        <MessagesSquare className="size-4 flex-shrink-0 text-muted-foreground" aria-hidden="true" />
        <h1 className="text-[15px] font-semibold leading-tight">Agents HUB</h1>
      </header>
      <div className="flex min-h-0 flex-1">
        <HubChannelList channels={channels} selected={current} />
        <HubThreadList channel={current} channelName={channels.find((c) => c.id === current)?.name ?? ""} />
        <HubConversation channel={current} />
      </div>
    </div>
  );
}
