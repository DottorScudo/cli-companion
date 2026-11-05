import { useState } from "react";
import { Loader2, ShieldQuestion } from "lucide-react";
import { openManager } from "../../app/actions/view";
import { resolveActionRequest } from "../../app/actions/hub";
import { useStore } from "../../hooks/useStore";
import { formatRelative } from "../../lib/board";
import { errorMessage } from "../../lib/errors";
import {
  REQUEST_CANCEL_LABEL,
  REQUEST_CONFIRM_LABEL,
  REQUEST_DEPENDENTS_LABEL,
  displayState,
  requestActionLabel,
  requestHeading,
  requestStateLabel,
} from "../../lib/hubActionRequests";
import type { DisplayState } from "../../lib/hubActionRequests";
import { taskLinkLabel } from "../../lib/hubProposals";
import type { HubActionRequest } from "../../bindings/HubActionRequest";
import type { HubEvent, HubMessage } from "../../lib/tauri";
import { cn } from "../../lib/utils";
import { hubStore } from "../../stores/hub";
import { Button } from "../ui/button";
import { useEventLinks } from "./useEventLinks";

const BADGE_CLASS: Record<DisplayState, string> = {
  pending: "border-status-attention/60 text-status-attention",
  confirmed: "border-status-finished/60 text-status-finished",
  cancelled: "border-border-strong text-muted-foreground",
  expired: "border-border-strong text-muted-foreground",
  stale: "border-border-strong text-muted-foreground",
  failed: "border-destructive/60 text-destructive",
};

interface Props {
  message: HubMessage;
  event: HubEvent;
  request: HubActionRequest;
  now: number;
}

/**
 * Richiesta di un'azione irreversibile fatta da un segretario, come scheda:
 * chi la chiede, azione, task, motivo, task che una cascata toglierebbe e stato.
 * Finché aspetta l'utente (e non è scaduta) ha «Conferma» e «Annulla»: senza
 * il suo clic il segretario non fa nulla.
 */
export default function HubActionRequestCard({ message, event, request, now }: Props) {
  const channel = useStore(hubStore, (s) => s.channel);
  const threadId = useStore(hubStore, (s) => s.threadId);
  const { task } = useEventLinks(event);
  const [sending, setSending] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  const state = displayState(request, now);
  const pending = state === "pending";
  const heading = requestHeading(request);
  const { workspace } = event;

  const decide = async (confirm: boolean) => {
    if (threadId === null || sending !== null) return;
    setError(null);
    setSending(confirm);
    try {
      await resolveActionRequest(channel, threadId, message.id, confirm);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setSending(null);
    }
  };

  return (
    <article
      aria-label={heading}
      className={cn(
        "flex min-w-0 flex-col gap-2 rounded-[10px] border p-3",
        pending ? "border-status-attention/60 bg-status-attention/5" : "bg-card",
      )}
    >
      <header className="flex items-center gap-2">
        <ShieldQuestion className="!size-3.5 flex-shrink-0 text-status-attention" aria-hidden="true" />
        <h3 className="min-w-0 truncate text-[13px] font-semibold">{heading}</h3>
        <span className="flex-1" />
        <span className={cn("flex-shrink-0 rounded-full border px-2 py-px text-[11px]", BADGE_CLASS[state])}>
          {requestStateLabel(state)}
        </span>
        <span className="flex-shrink-0 text-[11px] text-muted-foreground">
          {formatRelative(message.created_at, now)}
        </span>
      </header>
      <p className="flex min-w-0 items-baseline gap-2 text-[13px]">
        <span className="min-w-0 truncate font-medium" title={request.task_title}>
          {requestActionLabel(request.action)}: «{request.task_title}»
        </span>
        {event.task_id !== null && (
          <span className="min-w-0 truncate font-mono text-[11px] text-muted-foreground" title={event.task_id}>
            {event.task_id}
          </span>
        )}
      </p>
      <p className="whitespace-pre-line break-words text-[13px]">{request.reason}</p>
      {request.dependents.length > 0 && (
        <div className="text-[12px] text-text-secondary">
          <p>{REQUEST_DEPENDENTS_LABEL}:</p>
          <ul className="list-inside list-disc break-words">
            {request.dependents.map((title, i) => (
              <li key={`${i}-${title}`}>{title}</li>
            ))}
          </ul>
        </div>
      )}
      {request.note !== null && <p className="break-words text-[12px] text-text-secondary">{request.note}</p>}
      {error !== null && (
        <p role="alert" className="text-[11px] text-destructive">
          {error}
        </p>
      )}
      {(pending || (task && workspace !== null)) && (
        <div className="flex items-center gap-3">
          {pending && (
            <div className="flex items-center gap-1.5">
              <Button
                type="button"
                size="sm"
                variant="destructive"
                disabled={sending !== null}
                onClick={() => void decide(true)}
                className="h-7 gap-1.5 px-2.5 text-xs"
              >
                {sending === true && <Loader2 className="!size-3 animate-spin motion-reduce:animate-none" />}
                {REQUEST_CONFIRM_LABEL}
              </Button>
              <Button
                type="button"
                size="sm"
                variant="outline"
                disabled={sending !== null}
                onClick={() => void decide(false)}
                className="h-7 gap-1.5 px-2.5 text-xs"
              >
                {sending === false && <Loader2 className="!size-3 animate-spin motion-reduce:animate-none" />}
                {REQUEST_CANCEL_LABEL}
              </Button>
            </div>
          )}
          {task && workspace !== null && (
            <button
              type="button"
              onClick={() => openManager(workspace, { taskId: task.id })}
              className="text-[11px] text-primary hover:underline"
            >
              {taskLinkLabel(task)}
            </button>
          )}
        </div>
      )}
    </article>
  );
}
