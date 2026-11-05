import { formatRelative } from "../../lib/board";
import type { HubMessage } from "../../lib/tauri";
import Markdown from "../tasks/Markdown";
import HubMessageAttachments from "./HubMessageAttachments";
import ReplyReference from "./ReplyReference";

interface Props {
  channel: string;
  threadId: string;
  message: HubMessage;
  /** Il messaggio a cui questo risponde, se esiste ancora. */
  target: HubMessage | null;
  now: number;
  /** Nomi dei segretari, per evidenziare le menzioni nel testo. */
  mentions: readonly string[];
}

/**
 * Messaggio dell'utente: a destra, come in una chat, in una bolla al massimo
 * larga tre quarti della conversazione. Niente nome né avatar (è chi legge);
 * ora, riferimento della risposta e allegati (sotto il testo) stanno nella bolla.
 */
export default function HubUserMessage({ channel, threadId, message, target, now, mentions }: Props) {
  return (
    <article
      aria-label="Il tuo messaggio"
      className="flex min-w-0 max-w-[75%] flex-col gap-1 self-end rounded-lg rounded-br-sm border bg-surface-2 px-3 py-2"
    >
      {target && <ReplyReference target={target} />}
      {message.body !== "" && <Markdown source={message.body} mentions={mentions} />}
      <HubMessageAttachments channel={channel} threadId={threadId} attachments={message.attachments} />
      <span className="self-end text-[11px] text-muted-foreground">{formatRelative(message.created_at, now)}</span>
    </article>
  );
}
