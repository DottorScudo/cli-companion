import { useEffect, useMemo, useRef } from "react";
import { startReply } from "../../app/actions/hub";
import { openManager } from "../../app/actions/view";
import { useStore } from "../../hooks/useStore";
import { GENERAL_CHANNEL, replyTarget } from "../../lib/hub";
import { cardStatus, proposalStatuses } from "../../lib/hubProposals";
import { deliveryFailures, questionStates } from "../../lib/hubQuestions";
import { secretaryNames } from "../../lib/hubSecretaries";
import { hubStore } from "../../stores/hub";
import { useManagerNow } from "../tasks/useManagerNow";
import type { HubSecretary } from "../../lib/tauri";
import { Button } from "../ui/button";
import HubComposer from "./HubComposer";
import HubMessageItem from "./HubMessageItem";
import ThreadOwner from "./ThreadOwner";
import ThreadSession from "./ThreadSession";
import WorkspaceTerminalsButton from "./WorkspaceTerminalsButton";
import TypingIndicator from "./TypingIndicator";

const NO_SECRETARIES: readonly HubSecretary[] = [];

/** Quanto sotto il fondo si può essere e restare "agganciati" ai messaggi nuovi. */
const STICK_TO_BOTTOM_PX = 80;

function EmptyState({ children }: { children: string }) {
  return <p className="m-auto max-w-xs px-6 text-center text-xs text-muted-foreground">{children}</p>;
}

interface Props {
  channel: string;
  channelName: string;
  /** Vista messenger: workspace e «Terminali» nell'intestazione, messaggi dei segretari come bolle. */
  messenger: boolean;
}

/** Colonna della conversazione del thread selezionato, con il compositore in fondo. */
export default function HubConversation({ channel, channelName, messenger }: Props) {
  const threadId = useStore(hubStore, (s) => s.threadId);
  const thread = useStore(hubStore, (s) => s.thread);
  const settings = useStore(hubStore, (s) => s.settings);
  const secretaries = settings?.secretaries ?? NO_SECRETARIES;
  const mentions = useMemo(() => secretaryNames(secretaries), [secretaries]);
  const messages = thread?.messages;
  const statuses = useMemo(() => proposalStatuses(messages ?? []), [messages]);
  const questions = useMemo(() => questionStates(messages ?? []), [messages]);
  const failures = useMemo(() => deliveryFailures(messages ?? []), [messages]);
  const now = useManagerNow();
  const listRef = useRef<HTMLDivElement>(null);
  const stuckRef = useRef(true);

  // Un altro thread parte dal fondo; nello stesso thread si segue il fondo solo se ci si era già.
  useEffect(() => {
    stuckRef.current = true;
  }, [threadId]);
  useEffect(() => {
    const el = listRef.current;
    if (el && stuckRef.current) el.scrollTop = el.scrollHeight;
  }, [thread]);

  if (threadId === null) {
    return (
      <section aria-label="Conversazione" className="flex min-w-0 flex-1 flex-col">
        <EmptyState>
          {messenger
            ? "Scegli una chat dall’elenco, oppure creane una con +"
            : "Scegli un thread dall’elenco, oppure creane uno nuovo."}
        </EmptyState>
      </section>
    );
  }
  if (thread === null) {
    return (
      <section aria-label="Conversazione" className="flex min-w-0 flex-1 flex-col">
        <EmptyState>Caricamento…</EmptyState>
      </section>
    );
  }

  return (
    <section aria-label="Conversazione" className="flex min-w-0 flex-1 flex-col">
      <header className="flex items-center gap-2 border-b px-4 py-2">
        <h2 className="min-w-0 flex-1 truncate text-[13px] font-semibold">{thread.title}</h2>
        {messenger && (
          <>
            <span className="flex-shrink-0 text-[11px] text-muted-foreground">{channelName}</span>
            <WorkspaceTerminalsButton channel={channel} channelName={channelName} />
          </>
        )}
        <ThreadOwner owner={thread.owner} />
        <ThreadSession channel={channel} threadId={thread.id} now={now} />
        <span className="rounded border px-1.5 py-px text-[10px] text-muted-foreground">
          {thread.state === "open" ? "Aperto" : "Chiuso"}
        </span>
        {channel !== GENERAL_CHANNEL && (
          <Button type="button" size="sm" variant="outline" className="h-7 px-2" onClick={() => openManager(channel)}>
            Apri nel Tasks Manager
          </Button>
        )}
      </header>
      <div
        ref={listRef}
        onScroll={(e) => {
          const el = e.currentTarget;
          stuckRef.current = el.scrollHeight - el.scrollTop - el.clientHeight < STICK_TO_BOTTOM_PX;
        }}
        className="flex min-h-0 flex-1 flex-col gap-2 overflow-y-auto px-4 py-3"
      >
        {thread.messages.length === 0 ? (
          <EmptyState>Nessun messaggio: scrivi il primo.</EmptyState>
        ) : (
          thread.messages.map((m) => (
            <HubMessageItem
              key={m.id}
              channel={channel}
              threadId={thread.id}
              message={m}
              target={replyTarget(thread, m.reply_to)}
              now={now}
              secretaries={secretaries}
              mentions={mentions}
              bubbles={messenger}
              proposalStatus={cardStatus(m, statuses)}
              questionState={questions[m.id]}
              failedQuestion={failures[m.id]}
              onReply={startReply}
            />
          ))
        )}
      </div>
      <TypingIndicator channel={channel} threadId={thread.id} />
      <HubComposer key={thread.id} channel={channel} thread={thread} />
    </section>
  );
}
