import { useMemo, useState } from "react";
import { Cable } from "lucide-react";
import { useStore } from "../../hooks/useStore";
import { descriptorsOfKind } from "../../lib/connectors";
import { routesToThread } from "../../lib/connectorRoutes";
import { connectorsStore } from "../../stores/connectors";
import { Button } from "../ui/button";
import LinkedSourcesDialog from "./LinkedSourcesDialog";

/** «Fonti collegate» nell'intestazione del thread: le regole che portano lì elementi di una fonte. Niente se non ci sono fonti. */
export default function LinkedSourcesButton({ channel, threadId }: { channel: string; threadId: string }) {
  const [open, setOpen] = useState(false);
  const hasSources = useStore(connectorsStore, (s) => descriptorsOfKind(s.descriptors, "source").length > 0);
  const routes = useStore(connectorsStore, (s) => s.routes);
  const linked = useMemo(() => routesToThread(routes, channel, threadId).length, [routes, channel, threadId]);
  if (!hasSources && linked === 0) return null;
  return (
    <>
      <Button
        type="button"
        size="sm"
        variant="outline"
        className="h-7 gap-1 px-2"
        onClick={() => setOpen(true)}
        title="Le fonti esterne che portano elementi in questo thread"
      >
        <Cable className="!size-3.5" aria-hidden="true" /> Fonti collegate{linked > 0 ? ` (${linked})` : ""}
      </Button>
      {open && <LinkedSourcesDialog channel={channel} threadId={threadId} onClose={() => setOpen(false)} />}
    </>
  );
}
