import {
  AlertTriangle,
  CheckCircle2,
  CircleDot,
  FileText,
  Play,
  Reply,
  Trash2,
  XCircle,
  type LucideIcon,
} from "lucide-react";
import { formatRelative } from "../../lib/board";
import { eventLook } from "../../lib/hubEvents";
import type { HubEventIcon, HubEventTone } from "../../lib/hubEvents";
import type { HubEvent, HubMessage } from "../../lib/tauri";
import { cn } from "../../lib/utils";
import { Button } from "../ui/button";
import ReplyReference from "./ReplyReference";

const ICONS: Record<HubEventIcon, LucideIcon> = {
  proposal: FileText,
  started: Play,
  done: CheckCircle2,
  failed: XCircle,
  attention: AlertTriangle,
  deleted: Trash2,
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

/** Evento della board come riga compatta: icona e tipo, testo, nota del riassunto e "Rispondi". */
export default function HubEventRow({ message, event, target, now, onReply }: Props) {
  const look = eventLook(event.kind);
  const Icon = ICONS[look.icon];
  return (
    <div className="group flex items-start gap-2 rounded-md px-2 py-1.5 hover:bg-accent/40">
      <Icon className={cn("mt-0.5 !size-4 flex-shrink-0", TONE_CLASS[look.tone])} aria-hidden="true" />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <p className="text-[12px] leading-snug">
          <span className={cn("font-semibold", TONE_CLASS[look.tone])}>{look.label}</span>
          <span className="text-text-secondary"> · {message.body}</span>
        </p>
        {event.summary && <p className="whitespace-pre-line text-[11px] text-muted-foreground">{event.summary}</p>}
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
