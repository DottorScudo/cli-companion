import { useState } from "react";
import { Plus } from "lucide-react";
import { canCreateInstance, instancesOfConnector } from "../../../lib/connectors";
import type { ConnectorDescriptor, ConnectorInstanceView } from "../../../lib/tauri";
import { Button } from "../../ui/button";
import InstanceCard from "./InstanceCard";
import InstanceForm from "./InstanceForm";

/** Un connettore disponibile: nome, descrizione, le sue istanze e «Aggiungi». */
export default function DescriptorGroup({
  descriptor,
  instances,
}: {
  descriptor: ConnectorDescriptor;
  instances: readonly ConnectorInstanceView[];
}) {
  const [creating, setCreating] = useState(false);
  const own = instancesOfConnector(instances, descriptor.id);

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-start gap-3">
        <div className="min-w-0 flex-1">
          <h3 className="text-[13px] font-semibold">{descriptor.label}</h3>
          <p className="text-[11px] text-muted-foreground">{descriptor.description}</p>
        </div>
        {!creating && (
          <Button
            size="sm"
            variant="outline"
            className="gap-1"
            disabled={!canCreateInstance(instances)}
            title={canCreateInstance(instances) ? undefined : "Hai raggiunto il numero massimo di connettori"}
            onClick={() => setCreating(true)}
          >
            <Plus aria-hidden="true" /> Aggiungi
          </Button>
        )}
      </div>
      {creating && (
        <InstanceForm
          descriptor={descriptor}
          view={null}
          onDone={() => setCreating(false)}
          onCancel={() => setCreating(false)}
        />
      )}
      {own.map((view) => (
        <InstanceCard key={view.instance.id} view={view} descriptor={descriptor} />
      ))}
      {own.length === 0 && !creating && (
        <p className="text-[11px] text-muted-foreground">Nessuna istanza configurata.</p>
      )}
    </div>
  );
}
