import {
  AlertTriangle,
  CheckCircle2,
  CircleDot,
  FileText,
  MessageCircleQuestion,
  Play,
  ShieldQuestion,
  Reply,
  Trash2,
  XCircle,
  type LucideIcon,
} from "lucide-react";
import { openManager } from "../../app/actions/view";
import { formatRelative } from "../../lib/board";
import { eventLook } from "../../lib/hubEvents";
import { taskLinkLabel } from "../../lib/hubProposals";
import type { HubEventIcon, HubEventTone } from "../../lib/hubEvents";
import type { HubEvent, HubMessage } from "../../lib/tauri";
import { cn } from "../../lib/utils";
import { Button } from "../ui/button";
import ReplyReference from "./ReplyReference";
import { useEventLinks } from "./useEventLinks";
import WorkerTerminalLink from "./WorkerTerminalLink";

const ICONS: Record<HubEventIcon, LucideIcon> = {
  proposal: FileText,
  started: Play,
  done: CheckCircle2,
  failed: XCircle,
  attention: AlertTriangle,
  question: MessageCircleQuestion,
  deleted: Trash2,
  request: ShieldQuestion,
  generic: CircleDot,
};

const TONE_CLASS: Record<HubEventTone, string> = {
  neutral: "text-muted-foreground",
  success: "text-status-finished",
  danger: "text-destructive",
  warning: "text-status-attention",
};

interface Props {
  message: HubMessage;
  event: HubEvent;
  target: HubMessage | null;
  now: number;
  onReply: (messageId: string) => void;
}

/**
 * Evento della board come riga compatta: icona e tipo, testo, link al terminale
 * del worker (se il tab è ancora aperto), link al task nel Tasks Manager (se è
 * ancora nella board) e "Rispondi".
 */
export default function HubEventRow({ message, event, target, now, onReply }: Props) {
  const look = eventLook(event.kind);
  const Icon = ICONS[look.icon];
  const { workerTab, task } = useEventLinks(event);
  const { workspace } = event;
  return (
    <div className="group flex items-start gap-2 rounded-md px-2 py-1.5 hover:bg-accent/40">
      <Icon className={cn("mt-0.5 !size-4 flex-shrink-0", TONE_CLASS[look.tone])} aria-hidden="true" />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <p className="whitespace-pre-line text-[12px] leading-snug">
          <span className={cn("font-semibold", TONE_CLASS[look.tone])}>{look.label}</span>
          <span className="text-text-secondary"> · {message.body}</span>
        </p>
        {workerTab !== null && <WorkerTerminalLink tab={workerTab} />}
        {task && workspace !== null && (
          <button
            type="button"
            onClick={() => openManager(workspace, { taskId: task.id })}
            className="self-start text-[11px] text-primary hover:underline"
          >
            {taskLinkLabel(task)}
          </button>
        )}
        {target && <ReplyReference target={target} />}
      </div>
      <span className="flex-shrink-0 pt-0.5 text-[11px] text-muted-foreground">
        {formatRelative(message.created_at, now)}
      </span>
      <Button
        type="button"
        variant="ghost"
        size="sm"
        onClick={() => onReply(message.id)}
        className="h-6 flex-shrink-0 gap-1 px-1.5 text-[11px] text-muted-foreground hover:text-foreground"
      >
        <Reply className="!size-3" aria-hidden="true" /> Rispondi
      </Button>
    </div>
  );
}
