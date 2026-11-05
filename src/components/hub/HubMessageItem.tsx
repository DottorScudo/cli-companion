import { memo } from "react";
import { formatRelative } from "../../lib/board";
import { authorLabel } from "../../lib/hub";
import { findSecretary } from "../../lib/hubSecretaries";
import type { HubEvent, HubMessage, HubSecretary } from "../../lib/tauri";
import BotAvatar from "../BotAvatar";
import Markdown from "../tasks/Markdown";
import type { ProposalStatus } from "../../lib/hubProposals";
import type { QuestionState } from "../../lib/hubQuestions";
import HubDeliveryFailure from "./HubDeliveryFailure";
import HubEventRow from "./HubEventRow";
import HubQuestionCard from "./HubQuestionCard";
import HubProposalCard from "./HubProposalCard";
import HubUserMessage from "./HubUserMessage";
import ReplyReference from "./ReplyReference";
import SecretaryBadge from "./SecretaryBadge";

const AVATAR_SIZE = 28;

interface Props {
  message: HubMessage;
  /** Il messaggio a cui questo risponde, se esiste ancora. */
  target: HubMessage | null;
  now: number;
  /** Segretari configurati: per l'immagine di chi scrive. */
  secretaries: readonly HubSecretary[];
  /** Nomi dei segretari, per evidenziare le menzioni nel testo. */
  mentions: readonly string[];
  /** Stato della proposta del messaggio «proposta pronta»; assente negli altri. */
  proposalStatus?: ProposalStatus;
  /** Stato della domanda del messaggio «domanda di un worker»; assente negli altri. */
  questionState?: QuestionState;
  /** Se il messaggio è l'esito «Risposta non inviata», l'evento della domanda a cui si riferisce. */
  failedQuestion?: HubEvent;
  onReply: (messageId: string) => void;
}

/**
 * Un messaggio della conversazione: testo dell'utente (bolla a destra),
 * risposta di un segretario (avatar, nome e badge) o del sistema, oppure
 * evento in forma compatta, o scheda (proposta, domanda di un worker); tutti
 * tranne l'utente stanno a sinistra.
 */
function HubMessageItem({
  message,
  target,
  now,
  secretaries,
  mentions,
  proposalStatus,
  questionState,
  failedQuestion,
  onReply,
}: Props) {
  const proposalId = message.event?.proposal_id;
  if (message.event && proposalId && proposalStatus) {
    return (
      <HubProposalCard
        message={message}
        event={message.event}
        proposalId={proposalId}
        status={proposalStatus}
        target={target}
        now={now}
        onReply={onReply}
      />
    );
  }
  const question = message.event?.question;
  if (message.event && question && questionState) {
    return (
      <HubQuestionCard
        message={message}
        event={message.event}
        question={question}
        state={questionState}
        target={target}
        now={now}
      />
    );
  }
  if (failedQuestion) {
    return <HubDeliveryFailure message={message} question={failedQuestion} target={target} now={now} />;
  }
  if (message.event) {
    return <HubEventRow message={message} event={message.event} target={target} now={now} onReply={onReply} />;
  }
  const { author } = message;
  if (author.type === "user") {
    return <HubUserMessage message={message} target={target} now={now} mentions={mentions} />;
  }
  const isSecretary = author.type === "bot";
  const secretary = author.type === "bot" ? findSecretary(secretaries, author.name) : undefined;
  return (
    <article
      className={
        isSecretary
          ? "flex gap-2.5 rounded-lg border border-primary/30 bg-primary/5 px-3 py-2"
          : "flex flex-col gap-1 rounded-lg border bg-card px-3 py-2"
      }
    >
      {author.type === "bot" && (
        <BotAvatar name={author.name} size={AVATAR_SIZE} image={secretary?.image ?? undefined} className="mt-0.5" />
      )}
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <header className="flex items-baseline gap-2">
          <span className="text-[12px] font-semibold">
            {isSecretary ? `@${authorLabel(author)}` : authorLabel(author)}
          </span>
          {isSecretary && <SecretaryBadge />}
          <span className="text-[11px] text-muted-foreground">{formatRelative(message.created_at, now)}</span>
        </header>
        {target && <ReplyReference target={target} />}
        <Markdown source={message.body} mentions={mentions} />
      </div>
    </article>
  );
}

export default memo(HubMessageItem);
