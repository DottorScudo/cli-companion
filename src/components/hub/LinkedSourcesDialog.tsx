import { useEffect, useMemo, useState } from "react";
import { openConnectorSettings } from "../../app/actions/settings";
import { reloadRoutes } from "../../app/actions/connectors";
import { useStore } from "../../hooks/useStore";
import { findDescriptor, instancesOfKind } from "../../lib/connectors";
import { routesToThread } from "../../lib/connectorRoutes";
import { connectorsStore } from "../../stores/connectors";
import RouteEditor from "../settings/connectors/RouteEditor";
import RouteRow from "../settings/connectors/RouteRow";
import { selectClass } from "../settings/shared";
import { Button } from "../ui/button";
import { Dialog, DialogBody, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "../ui/dialog";

interface Props {
  channel: string;
  threadId: string;
  onClose: () => void;
}

/** Le regole delle fonti verso questo thread: si vedono, se ne aggiunge una (scelta dell'istanza e filtro) o si toglie. */
export default function LinkedSourcesDialog({ channel, threadId, onClose }: Props) {
  const routes = useStore(connectorsStore, (s) => s.routes);
  const descriptors = useStore(connectorsStore, (s) => s.descriptors);
  const instances = useStore(connectorsStore, (s) => s.instances);
  const sources = useMemo(() => instancesOfKind(instances, descriptors, "source"), [instances, descriptors]);
  const linked = useMemo(() => routesToThread(routes, channel, threadId), [routes, channel, threadId]);
  const [adding, setAdding] = useState(false);
  const [instanceId, setInstanceId] = useState("");

  // Una regola si disabilita da sola se il suo thread sparisce: all'apertura si rileggono.
  useEffect(() => {
    void reloadRoutes();
  }, []);

  const chosen = sources.find((s) => s.instance.id === instanceId) ?? sources[0];
  const descriptor = chosen && findDescriptor(descriptors, chosen.instance.connector);
  const labelOf = (id: string) => sources.find((s) => s.instance.id === id)?.instance.label ?? "Fonte eliminata";
  const finish = () => setAdding(false);

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="inset-auto left-1/2 top-1/2 max-h-[85vh] w-[34rem] max-w-[calc(100vw-2rem)] -translate-x-1/2 -translate-y-1/2">
        <DialogHeader>
          <DialogTitle>Fonti collegate</DialogTitle>
          <DialogDescription>Le fonti che portano i loro elementi in questo thread.</DialogDescription>
        </DialogHeader>
        <DialogBody>
          {linked.length === 0 && !adding && (
            <p className="text-xs text-muted-foreground">Nessuna fonte collegata a questo thread.</p>
          )}
          <ul className="flex flex-col gap-1.5" aria-label="Regole verso questo thread">
            {linked.map((route) => (
              <RouteRow key={route.id} route={route} context={labelOf(route.instance)} />
            ))}
          </ul>
          {adding && chosen && descriptor && (
            <div className="flex flex-col gap-3">
              <div>
                <label htmlFor="linked-source-instance" className="mb-1 block text-xs font-semibold">
                  Fonte
                </label>
                <select
                  id="linked-source-instance"
                  value={chosen.instance.id}
                  onChange={(e) => setInstanceId(e.target.value)}
                  className={selectClass}
                >
                  {sources.map((s) => (
                    <option key={s.instance.id} value={s.instance.id}>
                      {s.instance.label}
                    </option>
                  ))}
                </select>
              </div>
              <RouteEditor
                key={chosen.instance.id}
                instanceId={chosen.instance.id}
                descriptor={descriptor}
                route={null}
                locked={{ channel, threadId }}
                onDone={finish}
                onCancel={finish}
              />
            </div>
          )}
          {!adding && sources.length > 0 && (
            <div>
              <Button size="sm" variant="outline" onClick={() => setAdding(true)}>
                Collega una fonte
              </Button>
            </div>
          )}
          {sources.length === 0 && (
            <div className="flex flex-col items-start gap-2">
              <p className="text-xs text-muted-foreground">
                Non c’è ancora nessuna fonte configurata: creane una nelle impostazioni.
              </p>
              <Button
                size="sm"
                variant="outline"
                onClick={() => {
                  onClose();
                  openConnectorSettings();
                }}
              >
                Apri le impostazioni dei connettori
              </Button>
            </div>
          )}
        </DialogBody>
      </DialogContent>
    </Dialog>
  );
}
