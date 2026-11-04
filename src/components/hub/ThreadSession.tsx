import { useStore } from "../../hooks/useStore";
import { expiryText, findSecretary, threadSession } from "../../lib/hubSecretaries";
import { hubStore } from "../../stores/hub";
import BotAvatar from "../BotAvatar";

const AVATAR_SIZE = 18;

interface Props {
  channel: string;
  threadId: string;
  now: number;
}

/** Sessione attiva sul thread (il segretario sveglio) con il tempo che resta. Niente se nessuno è sveglio; può non essere il titolare. */
export default function ThreadSession({ channel, threadId, now }: Props) {
  const session = useStore(hubStore, (s) => threadSession(s.secretaryStatus, channel, threadId));
  const secretaries = useStore(hubStore, (s) => s.settings?.secretaries);
  if (session === null) return null;
  const image = findSecretary(secretaries ?? [], session.secretary)?.image ?? undefined;
  return (
    <div
      className="flex flex-shrink-0 items-center gap-1.5 rounded border border-primary/40 bg-primary/10 px-1.5 py-0.5 text-[11px]"
      title="Il segretario con la sessione aperta sul thread"
    >
      <BotAvatar name={session.secretary} size={AVATAR_SIZE} image={image} />
      <span className="font-mono font-medium">@{session.secretary}</span>
      <span className="text-muted-foreground">
        · {session.working ? "sta lavorando" : expiryText(session.expires_at, now)}
      </span>
    </div>
  );
}
