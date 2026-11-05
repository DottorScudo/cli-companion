import { memo } from "react";
import type { HubEvent, HubMessage, HubSecretary } from "../../lib/tauri";
import type { ProposalStatus } from "../../lib/hubProposals";
import type { QuestionState } from "../../lib/hubQuestions";
import HubBotBubble from "./HubBotBubble";
import HubBotMessage from "./HubBotMessage";
import HubDeliveryFailure from "./HubDeliveryFailure";
import HubEventRow from "./HubEventRow";
import HubQuestionCard from "./HubQuestionCard";
import HubProposalCard from "./HubProposalCard";
import HubUserMessage from "./HubUserMessage";

interface Props {
  channel: string;
  threadId: string;
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
  /** Vista messenger: le risposte di segretari e sistema sono bolle a sinistra, non riquadri. */
  bubbles?: boolean;
  onReply: (messageId: string) => void;
}

/**
 * Un messaggio della conversazione: testo dell'utente (bolla a destra),
 * risposta di un segretario o del sistema (riquadro, o bolla nella vista messenger), oppure
 * evento in forma compatta, o scheda (proposta, domanda di un worker); tutti
 * tranne l'utente stanno a sinistra.
 */
function HubMessageItem({
  channel,
  threadId,
  message,
  target,
  now,
  secretaries,
  mentions,
  proposalStatus,
  questionState,
  failedQuestion,
  bubbles = false,
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
    return (
      <HubUserMessage
        channel={channel}
        threadId={threadId}
        message={message}
        target={target}
        now={now}
        mentions={mentions}
      />
    );
  }
  const Bot = bubbles ? HubBotBubble : HubBotMessage;
  return (
    <Bot
      channel={channel}
      threadId={threadId}
      message={message}
      target={target}
      now={now}
      secretaries={secretaries}
      mentions={mentions}
    />
  );
}

export default memo(HubMessageItem);
