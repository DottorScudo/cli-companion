import { useState } from "react";
import { deleteInstance, setInstanceEnabled } from "../../../app/actions/connectors";
import { canToggle } from "../../../lib/connectors";
import type { ConnectorDescriptor, ConnectorInstanceView } from "../../../lib/tauri";
import { Button } from "../../ui/button";
import { Section } from "../shared";
import InstanceActions from "./InstanceActions";
import InstanceForm from "./InstanceForm";
import RoutesList from "./RoutesList";
import SecretFields from "./SecretFields";
import StatusChip from "./StatusChip";
import { useOperation } from "./useOperation";

const SOURCE_KIND = "source";

/** Un'istanza: etichetta, interruttore, stato; sotto, modifica, segreti, azioni, regole ed eliminazione. */
export default function InstanceCard({
  view,
  descriptor,
}: {
  view: ConnectorInstanceView;
  /** `undefined` se il connettore non è compilato in questa versione. */
  descriptor: ConnectorDescriptor | undefined;
}) {
  const [editing, setEditing] = useState(false);
  const [confirming, setConfirming] = useState(false);
  const { run, busy, error } = useOperation();
  const { instance, status } = view;

  return (
    <Section className="gap-2.5 bg-background/40">
      <div className="flex flex-wrap items-start gap-x-3 gap-y-1">
        <h4 className="min-w-0 flex-1 truncate text-[13px] font-semibold">{instance.label}</h4>
        <label className="flex items-center gap-1.5 text-xs">
          <input
            type="checkbox"
            checked={instance.enabled}
            disabled={busy || !canToggle(instance.enabled, status)}
            onChange={(e) => void run(() => setInstanceEnabled(instance.id, e.target.checked))}
            className="h-3.5 w-3.5 accent-primary"
          />
          Abilitato
        </label>
        <StatusChip status={status} />
      </div>

      {descriptor === undefined ? (
        <p className="text-[11px] text-muted-foreground">
          Questo connettore non è disponibile in questa versione dell’app: l’istanza resta salvata e si può solo
          eliminare.
        </p>
      ) : editing ? (
        <InstanceForm
          descriptor={descriptor}
          view={view}
          onDone={() => setEditing(false)}
          onCancel={() => setEditing(false)}
        />
      ) : (
        <>
          <SecretFields view={view} fields={descriptor.config_fields} />
          <InstanceActions descriptor={descriptor} view={view} />
          {descriptor.kind === SOURCE_KIND && <RoutesList instanceId={instance.id} descriptor={descriptor} />}
        </>
      )}

      {confirming ? (
        <div className="flex flex-col gap-1.5 rounded-md border border-destructive/40 p-2" role="alert">
          <p className="text-xs font-semibold">Eliminare «{instance.label}»?</p>
          <p className="text-xs">
            Si cancellano anche le sue regole o i suoi collegamenti e tutti i segreti salvati nel portachiavi.
          </p>
          <div className="flex items-center gap-1.5">
            <Button
              size="sm"
              variant="destructive"
              disabled={busy}
              onClick={() => void run(() => deleteInstance(instance.id))}
            >
              Elimina
            </Button>
            <Button size="sm" variant="secondary" disabled={busy} onClick={() => setConfirming(false)}>
              Annulla
            </Button>
          </div>
        </div>
      ) : (
        <div className="flex items-center gap-2">
          {descriptor !== undefined && !editing && (
            <Button size="sm" variant="outline" disabled={busy} onClick={() => setEditing(true)}>
              Modifica
            </Button>
          )}
          <Button size="sm" variant="outline" disabled={busy} onClick={() => setConfirming(true)}>
            Elimina
          </Button>
        </div>
      )}
      {error && (
        <p className="whitespace-pre-wrap text-xs text-destructive" role="alert">
          {error}
        </p>
      )}
    </Section>
  );
}
