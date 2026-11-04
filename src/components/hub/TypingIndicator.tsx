import { useStore } from "../../hooks/useStore";
import { typingLabel, typingNames } from "../../lib/hubSecretaries";
import { shallowEqual } from "../../lib/store";
import { hubStore } from "../../stores/hub";

interface Props {
  channel: string;
  threadId: string;
}

/** «<nome> sta scrivendo…» mentre un segretario lavora sul thread. L'area resta nella pagina per l'annuncio agli screen reader. */
export default function TypingIndicator({ channel, threadId }: Props) {
  const names = useStore(hubStore, (s) => typingNames(s.secretaryStatus, channel, threadId), shallowEqual);
  return (
    <p role="status" aria-live="polite" className="min-h-4 px-4 pb-1 text-[11px] italic text-muted-foreground">
      {typingLabel(names)}
    </p>
  );
}
