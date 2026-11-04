import { useStore } from "../../hooks/useStore";
import { findSecretary } from "../../lib/hubSecretaries";
import { hubStore } from "../../stores/hub";
import BotAvatar from "../BotAvatar";

const AVATAR_SIZE = 16;

/** Avatar del titolare nella riga di un thread dell'elenco; niente se non si conosce. */
export default function ListOwner({ name }: { name: string | null }) {
  const secretaries = useStore(hubStore, (s) => s.settings?.secretaries);
  if (name === null) return null;
  const image = findSecretary(secretaries ?? [], name)?.image ?? undefined;
  return (
    <span className="flex-shrink-0" role="img" aria-label={`Titolare: @${name}`} title={`Titolare: @${name}`}>
      <BotAvatar name={name} size={AVATAR_SIZE} image={image} />
    </span>
  );
}
