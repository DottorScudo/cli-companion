import { Inbox } from "lucide-react";
import { formatRelative } from "../../lib/board";
import { authorLabel, viaLabel } from "../../lib/hub";
import type { HubMessage, HubSecretary } from "../../lib/tauri";
import { findSecretary } from "../../lib/hubSecretaries";
import BotAvatar from "../BotAvatar";
import Markdown from "../tasks/Markdown";
import HubMessageAttachments from "./HubMessageAttachments";
import ReplyReference from "./ReplyReference";

const AVATAR_SIZE = 28;

interface Props {
  channel: string;
  threadId: string;
  message: HubMessage;
  /** Il messaggio a cui questo risponde, se esiste ancora. */
  target: HubMessage | null;
  now: number;
  secretaries: readonly HubSecretary[];
  mentions: readonly string[];
}

/**
 * Risposta di un segretario, del sistema o di un mittente esterno come bolla a sinistra (vista messenger),
 * speculare a quella dell'utente: avatar (o icona generica per l'esterno) a fianco, nome e ora in piccolo sopra il testo.
 */
export default function HubBotBubble({ channel, threadId, message, target, now, secretaries, mentions }: Props) {
  const { author } = message;
  const secretary = author.type === "bot" ? findSecretary(secretaries, author.name) : undefined;
  const name = author.type === "bot" ? `@${authorLabel(author)}` : authorLabel(author);
  return (
    <article aria-label={`Messaggio di ${name}`} className="flex min-w-0 max-w-[75%] items-end gap-2 self-start">
      {author.type === "bot" && (
        <BotAvatar name={author.name} size={AVATAR_SIZE} image={secretary?.image ?? undefined} />
      )}
      {author.type === "external" && (
        <span
          aria-hidden="true"
          className="flex flex-shrink-0 items-center justify-center rounded-full bg-surface-2 text-muted-foreground"
          style={{ width: AVATAR_SIZE, height: AVATAR_SIZE }}
        >
          <Inbox size={15} />
        </span>
      )}
      <div className="flex min-w-0 flex-col gap-1 rounded-lg rounded-bl-sm border bg-card px-3 py-2">
        <span className="flex items-baseline gap-2 text-[11px] text-muted-foreground">
          <span className="font-semibold text-foreground">{name}</span>
          {author.type === "external" && <span>{viaLabel(author.connector)}</span>}
          {formatRelative(message.created_at, now)}
        </span>
        {target && <ReplyReference target={target} />}
        {message.body !== "" && <Markdown source={message.body} mentions={mentions} />}
        <HubMessageAttachments channel={channel} threadId={threadId} attachments={message.attachments} />
      </div>
    </article>
  );
}
