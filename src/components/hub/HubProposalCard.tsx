import { useCallback } from "react";
import { Reply, Sparkles } from "lucide-react";
import { useStores } from "../../hooks/useStore";
import { formatRelative } from "../../lib/board";
import { dependencyLabels } from "../../lib/taskPanel";
import {
  PROPOSAL_REPLY_HINT,
  PROPOSAL_STATUS_LABEL,
  proposalCount,
  proposalSpecs,
  proposalTitle,
} from "../../lib/hubProposals";
import type { ProposalStatus } from "../../lib/hubProposals";
import type { HubEvent, HubMessage } from "../../lib/tauri";
import { cn } from "../../lib/utils";
import { boardsStore } from "../../stores/boards";
import { Button } from "../ui/button";
import HubProposalTask from "./HubProposalTask";
import ReplyReference from "./ReplyReference";

const STATUS_CLASS: Record<ProposalStatus, string> = {
  pending: "border-status-attention/60 text-status-attention",
  accepted: "border-status-finished/60 text-status-finished",
  rejected: "border-border-strong text-muted-foreground",
};

const BOARD_STORES = [boardsStore] as const;

interface Props {
  message: HubMessage;
  event: HubEvent;
  proposalId: string;
  /** Stato attuale della proposta nel thread. */
  status: ProposalStatus;
  target: HubMessage | null;
  now: number;
  onReply: (messageId: string) => void;
}

/**
 * Evento «proposta pronta» come scheda: titolo con il numero di task, stato, task
 * espandibili (letti dalla board finché la proposta è pendente) e «Rispondi».
 * La scheda non decide: accettare o scartare si fa rispondendo al segretario.
 */
export default function HubProposalCard({ message, event, proposalId, status, target, now, onReply }: Props) {
  const computeSpecs = useCallback(
    () => proposalSpecs(boardsStore.getState().boards, event.workspace, proposalId),
    [event.workspace, proposalId],
  );
  const specs = useStores(BOARD_STORES, computeSpecs);
  const title = proposalTitle(proposalCount(specs, message.body));
  const pending = status === "pending";
  return (
    <article
      aria-label={title}
      className={cn(
        "flex min-w-0 flex-col gap-2 rounded-[10px] border p-3",
        pending ? "border-primary/45 bg-primary/5" : "bg-card",
      )}
    >
      <header className="flex items-center gap-2">
        <Sparkles className="!size-3.5 flex-shrink-0 text-primary" aria-hidden="true" />
        <h3 className="min-w-0 flex-1 truncate text-[13px] font-semibold">{title}</h3>
        <span className={cn("rounded-full border px-2 py-px text-[11px]", STATUS_CLASS[status])}>
          {PROPOSAL_STATUS_LABEL[status]}
        </span>
        <span className="flex-shrink-0 text-[11px] text-muted-foreground">
          {formatRelative(message.created_at, now)}
        </span>
      </header>
      {specs !== null ? (
        <ul className="flex min-w-0 flex-col gap-1.5">
          {specs.map((spec) => (
            <HubProposalTask key={spec.id} spec={spec} dependencies={dependencyLabels(spec.depends_on, [], specs)} />
          ))}
        </ul>
      ) : (
        <p className="whitespace-pre-line break-words text-[12px] text-text-secondary">{message.body}</p>
      )}
      {target && <ReplyReference target={target} />}
      <div className="flex items-center gap-2">
        <Button
          type="button"
          size="sm"
          variant={pending ? "default" : "ghost"}
          onClick={() => onReply(message.id)}
          className="h-7 gap-1 px-2.5 text-xs"
        >
          <Reply className="!size-3.5" aria-hidden="true" /> Rispondi
        </Button>
        {pending && <span className="text-[11px] text-muted-foreground">{PROPOSAL_REPLY_HINT}</span>}
      </div>
    </article>
  );
}
