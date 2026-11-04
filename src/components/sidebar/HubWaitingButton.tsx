import { MessageSquare } from "lucide-react";
import { openHubChannel } from "../../app/actions/view";
import { useStore } from "../../hooks/useStore";
import { waitingCount, waitingText } from "../../lib/hub";
import { hubStore } from "../../stores/hub";

/** Thread dell'Agents HUB che aspettano l'utente nel canale del workspace; apre l'HUB su quel canale. Nascosto a zero (e con l'HUB spento). */
export default function HubWaitingButton({ id }: { id: string }) {
  const waiting = useStore(hubStore, (s) => waitingCount(s.threads[id]));
  if (waiting === 0) return null;
  const label = `${waitingText(waiting)}: apri il canale`;
  return (
    <button
      type="button"
      onClick={(e) => {
        e.stopPropagation();
        openHubChannel(id);
      }}
      aria-label={label}
      title={label}
      className="flex flex-shrink-0 items-center gap-1 rounded bg-status-attention/15 px-1.5 py-0.5 text-[11px] font-medium text-status-attention transition-colors hover:bg-status-attention/25"
    >
      <MessageSquare className="!size-3" aria-hidden="true" />
      {waiting}
    </button>
  );
}
