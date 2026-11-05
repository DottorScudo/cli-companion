import { formatRelative } from "../../lib/board";
import { authorLabel } from "../../lib/hub";
import type { HubMessage, HubSecretary } from "../../lib/tauri";
import { findSecretary } from "../../lib/hubSecretaries";
import BotAvatar from "../BotAvatar";
import Markdown from "../tasks/Markdown";
import ReplyReference from "./ReplyReference";
import SecretaryBadge from "./SecretaryBadge";

const AVATAR_SIZE = 28;

interface Props {
  message: HubMessage;
  /** Il messaggio a cui questo risponde, se esiste ancora. */
  target: HubMessage | null;
  now: number;
  secretaries: readonly HubSecretary[];
  mentions: readonly string[];
}

/** Risposta di un segretario (avatar, nome e badge) o del sistema, in un riquadro a sinistra (vista per canale). */
export default function HubBotMessage({ message, target, now, secretaries, mentions }: Props) {
  const { author } = message;
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
