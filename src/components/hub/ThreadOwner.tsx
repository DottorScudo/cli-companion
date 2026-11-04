import { useStore } from "../../hooks/useStore";
import { expiryText, findSecretary, threadOwner } from "../../lib/hubSecretaries";
import { hubStore } from "../../stores/hub";
import BotAvatar from "../BotAvatar";

const AVATAR_SIZE = 18;

interface Props {
  channel: string;
  threadId: string;
  now: number;
}

/** Titolare del thread (il segretario sveglio) con il tempo che resta alla sua sessione. Niente se nessuno è sveglio. */
export default function ThreadOwner({ channel, threadId, now }: Props) {
  const owner = useStore(hubStore, (s) => threadOwner(s.secretaryStatus, channel, threadId));
  const secretaries = useStore(hubStore, (s) => s.settings?.secretaries);
  if (owner === null) return null;
  const image = findSecretary(secretaries ?? [], owner.secretary)?.image ?? undefined;
  return (
    <div
      className="flex flex-shrink-0 items-center gap-1.5 rounded border border-primary/40 bg-primary/10 px-1.5 py-0.5 text-[11px]"
      title="Il segretario titolare del thread"
    >
      <BotAvatar name={owner.secretary} size={AVATAR_SIZE} image={image} />
      <span className="font-mono font-medium">@{owner.secretary}</span>
      <span className="text-muted-foreground">
        · {owner.working ? "sta lavorando" : expiryText(owner.expires_at, now)}
      </span>
    </div>
  );
}
