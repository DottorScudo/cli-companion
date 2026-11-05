import { formatRelative } from "../../lib/board";
import { authorLabel } from "../../lib/hub";
import type { HubMessage, HubSecretary } from "../../lib/tauri";
import { findSecretary } from "../../lib/hubSecretaries";
import BotAvatar from "../BotAvatar";
import Markdown from "../tasks/Markdown";
import ReplyReference from "./ReplyReference";

const AVATAR_SIZE = 28;

interface Props {
  message: HubMessage;
  /** Il messaggio a cui questo risponde, se esiste ancora. */
  target: HubMessage | null;
  now: number;
  secretaries: readonly HubSecretary[];
  mentions: readonly string[];
}

/**
 * Risposta di un segretario o del sistema come bolla a sinistra (vista messenger),
 * speculare a quella dell'utente: avatar a fianco, nome e ora in piccolo sopra il testo.
 */
export default function HubBotBubble({ message, target, now, secretaries, mentions }: Props) {
  const { author } = message;
  const secretary = author.type === "bot" ? findSecretary(secretaries, author.name) : undefined;
  const name = author.type === "bot" ? `@${authorLabel(author)}` : authorLabel(author);
  return (
    <article aria-label={`Messaggio di ${name}`} className="flex min-w-0 max-w-[75%] items-end gap-2 self-start">
      {author.type === "bot" && (
        <BotAvatar name={author.name} size={AVATAR_SIZE} image={secretary?.image ?? undefined} />
      )}
      <div className="flex min-w-0 flex-col gap-1 rounded-lg rounded-bl-sm border bg-card px-3 py-2">
        <span className="flex items-baseline gap-2 text-[11px] text-muted-foreground">
          <span className="font-semibold text-foreground">{name}</span>
          {formatRelative(message.created_at, now)}
        </span>
        {target && <ReplyReference target={target} />}
        <Markdown source={message.body} mentions={mentions} />
      </div>
    </article>
  );
}
