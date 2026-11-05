import { useEffect, useMemo, useState } from "react";
import { loadChannelThreads } from "../../../app/actions/hub";
import { useHubChannels } from "../../../hooks/useHubChannels";
import { useStore } from "../../../hooks/useStore";
import { destinationText, existingTargetChannels, routesOfInstance } from "../../../lib/connectorRoutes";
import type { ConnectorDescriptor, ConnectorRoute } from "../../../lib/tauri";
import { connectorsStore } from "../../../stores/connectors";
import { hubStore } from "../../../stores/hub";
import { Button } from "../../ui/button";
import RouteEditor from "./RouteEditor";
import RouteRow from "./RouteRow";

/** Editor aperto: una regola nuova (`null`) o quella in modifica. */
type Editing = ConnectorRoute | null | undefined;

/** Le regole di una fonte: l'elenco con la destinazione e l'aggiunta di una nuova. */
export default function RoutesList({
  instanceId,
  descriptor,
}: {
  instanceId: string;
  descriptor: ConnectorDescriptor;
}) {
  const allRoutes = useStore(connectorsStore, (s) => s.routes);
  const routes = useMemo(() => routesOfInstance(allRoutes, instanceId), [allRoutes, instanceId]);
  const threads = useStore(hubStore, (s) => s.threads);
  const { channels } = useHubChannels();
  const hubEnabled = useStore(hubStore, (s) => s.enabled);

  // I titoli dei thread di destinazione: si leggono gli elenchi dei canali che servono.
  useEffect(() => {
    if (!hubEnabled) return;
    for (const channel of existingTargetChannels(routes)) {
      if (threads[channel] === undefined) loadChannelThreads(channel);
    }
  }, [hubEnabled, routes, threads]);
  const [editing, setEditing] = useState<Editing>(undefined);
  const close = () => setEditing(undefined);

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between gap-2">
        <p className="text-xs font-semibold">Regole</p>
        {editing === undefined && (
          <Button size="sm" variant="outline" className="h-7 px-2" onClick={() => setEditing(null)}>
            Aggiungi regola
          </Button>
        )}
      </div>
      {routes.length === 0 && editing === undefined && (
        <p className="text-[11px] text-muted-foreground">
          Nessuna regola: finché non ne aggiungi una, questa fonte non porta nulla nei thread.
        </p>
      )}
      <ul className="flex flex-col gap-1.5" aria-label="Regole">
        {routes.map((route) =>
          editing?.id === route.id ? (
            <li key={route.id}>
              <RouteEditor
                instanceId={instanceId}
                descriptor={descriptor}
                route={route}
                onDone={close}
                onCancel={close}
              />
            </li>
          ) : (
            <RouteRow
              key={route.id}
              route={route}
              context={destinationText(route, channels, threads)}
              onEdit={() => setEditing(route)}
            />
          ),
        )}
      </ul>
      {editing === null && (
        <RouteEditor instanceId={instanceId} descriptor={descriptor} route={null} onDone={close} onCancel={close} />
      )}
    </div>
  );
}
