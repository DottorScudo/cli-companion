import { useStore } from "../../hooks/useStore";
import { findSecretary } from "../../lib/hubSecretaries";
import { hubStore } from "../../stores/hub";
import BotAvatar from "../BotAvatar";

const AVATAR_SIZE = 18;

/** Titolare del thread (`HubThread.owner`): il segretario che vi ha aperto l'ultima pianificazione. Niente se non c'è. */
export default function ThreadOwner({ owner }: { owner: string | null }) {
  const secretaries = useStore(hubStore, (s) => s.settings?.secretaries);
  if (owner === null) return null;
  const image = findSecretary(secretaries ?? [], owner)?.image ?? undefined;
  return (
    <div
      className="flex flex-shrink-0 items-center gap-1.5 rounded border px-1.5 py-0.5 text-[11px]"
      title="Il segretario titolare del thread: lo svegliano gli eventi della pianificazione"
    >
      <BotAvatar name={owner} size={AVATAR_SIZE} image={image} />
      <span className="font-mono font-medium">@{owner}</span>
      <span className="text-muted-foreground">· titolare</span>
    </div>
  );
}
