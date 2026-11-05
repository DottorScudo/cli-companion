import { useState } from "react";
import { AlertTriangle, CheckCircle2, Loader2, Reply } from "lucide-react";
import { answerQuestion, startReply } from "../../app/actions/hub";
import { useStore } from "../../hooks/useStore";
import { formatRelative } from "../../lib/board";
import { errorMessage } from "../../lib/errors";
import {
  QUESTION_ANSWERED_ELSEWHERE,
  QUESTION_FREE_REPLY_LABEL,
  questionHeading,
  questionStateLabel,
} from "../../lib/hubQuestions";
import type { QuestionState } from "../../lib/hubQuestions";
import type { HubQuestion } from "../../bindings/HubQuestion";
import type { HubEvent, HubMessage } from "../../lib/tauri";
import { cn } from "../../lib/utils";
import { hubStore } from "../../stores/hub";
import { Button } from "../ui/button";
import ReplyReference from "./ReplyReference";
import { useEventLinks } from "./useEventLinks";
import WorkerTerminalLink from "./WorkerTerminalLink";

const BADGE_CLASS: Record<QuestionState["kind"], string> = {
  pending: "border-status-attention/60 text-status-attention",
  answered: "border-status-finished/60 text-status-finished",
  closed: "border-border-strong text-muted-foreground",
};

interface Props {
  message: HubMessage;
  event: HubEvent;
  question: HubQuestion;
  state: QuestionState;
  target: HubMessage | null;
  now: number;
}

/**
 * Domanda di un worker (o del pianificatore) come scheda: testo, stato, opzioni
 * come pulsanti che rispondono subito, «Rispondi» per una risposta libera e il
 * link al terminale del worker. Risposta data, la scheda mostra cosa è stato
 * inviato e non è più cliccabile.
 */
export default function HubQuestionCard({ message, event, question, state, target, now }: Props) {
  const channel = useStore(hubStore, (s) => s.channel);
  const threadId = useStore(hubStore, (s) => s.threadId);
  const { workerTab, task } = useEventLinks(event);
  const [sending, setSending] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const heading = questionHeading(message.body, task);
  const pending = state.kind === "pending";

  const answer = async (option: string) => {
    if (threadId === null || sending !== null) return;
    setError(null);
    setSending(option);
    try {
      await answerQuestion(channel, threadId, message.id, option);
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
        <AlertTriangle className="!size-3.5 flex-shrink-0 text-status-attention" aria-hidden="true" />
        <h3 className="min-w-0 truncate text-[13px] font-semibold">{heading}</h3>
        {event.task_id !== null && (
          <span className="min-w-0 truncate font-mono text-[11px] text-muted-foreground" title={event.task_id}>
            {event.task_id}
          </span>
        )}
        <span className="flex-1" />
        <span className={cn("flex-shrink-0 rounded-full border px-2 py-px text-[11px]", BADGE_CLASS[state.kind])}>
          {questionStateLabel(state)}
        </span>
        <span className="flex-shrink-0 text-[11px] text-muted-foreground">
          {formatRelative(message.created_at, now)}
        </span>
      </header>
      <p className="whitespace-pre-line break-words text-[13px]">{question.text}</p>
      {target && <ReplyReference target={target} />}
      {state.kind === "answered" && (
        <p className="flex items-start gap-1.5 break-words text-[12px] text-text-secondary">
          <CheckCircle2 className="mt-0.5 !size-3.5 flex-shrink-0 text-status-finished" aria-hidden="true" />
          <span className="min-w-0 whitespace-pre-line">{state.answer ?? QUESTION_ANSWERED_ELSEWHERE}</span>
        </p>
      )}
      {pending && question.options.length > 0 && (
        <div role="group" aria-label="Risposte suggerite" className="flex flex-wrap gap-1.5">
          {question.options.map((option) => (
            <Button
              key={option}
              type="button"
              size="sm"
              variant="outline"
              disabled={sending !== null}
              onClick={() => void answer(option)}
              className="h-7 max-w-full gap-1.5 px-2.5 text-xs"
            >
              {sending === option && <Loader2 className="!size-3 animate-spin motion-reduce:animate-none" />}
              <span className="truncate">{option}</span>
            </Button>
          ))}
        </div>
      )}
      {error !== null && (
        <p role="alert" className="text-[11px] text-destructive">
          {error}
        </p>
      )}
      {(pending || workerTab !== null) && (
        <div className="flex items-center gap-3">
          {pending && (
            <Button
              type="button"
              size="sm"
              variant="ghost"
              disabled={sending !== null}
              onClick={() => startReply(message.id)}
              className="h-7 gap-1 px-2.5 text-xs"
            >
              <Reply className="!size-3.5" aria-hidden="true" /> {QUESTION_FREE_REPLY_LABEL}
            </Button>
          )}
          {workerTab !== null && <WorkerTerminalLink tab={workerTab} />}
        </div>
      )}
    </article>
  );
}
