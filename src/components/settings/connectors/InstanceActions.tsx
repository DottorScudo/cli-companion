import { useState } from "react";
import { runConnectorAction, testConnection } from "../../../app/actions/connectors";
import { PAIRING_CODE_NOTE, runnableActions } from "../../../lib/connectors";
import type { ConnectorActionOutcome, ConnectorDescriptor, ConnectorInstanceView } from "../../../lib/tauri";
import { Button } from "../../ui/button";
import { useOperation } from "./useOperation";

/** Le azioni del descrittore (autorizza, abbina…) e «Prova connessione», con il loro esito. */
export default function InstanceActions({
  descriptor,
  view,
}: {
  descriptor: ConnectorDescriptor;
  view: ConnectorInstanceView;
}) {
  const [outcome, setOutcome] = useState<ConnectorActionOutcome | null>(null);
  const { run, busy, error } = useOperation();
  const id = view.instance.id;

  const execute = (operation: () => Promise<ConnectorActionOutcome>) => {
    setOutcome(null);
    void run(async () => setOutcome(await operation()));
  };

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap gap-2">
        {runnableActions(descriptor).map((action) => (
          <Button
            key={action.id}
            size="sm"
            variant="outline"
            disabled={busy}
            title={action.help}
            onClick={() => execute(() => runConnectorAction(id, action.id))}
          >
            {action.label}
          </Button>
        ))}
        <Button size="sm" variant="outline" disabled={busy} onClick={() => execute(() => testConnection(id))}>
          Prova connessione
        </Button>
      </div>
      {error && (
        <p className="whitespace-pre-wrap text-xs text-destructive" role="alert">
          {error}
        </p>
      )}
      {outcome && (
        <div className="flex flex-col gap-1 rounded-md border bg-muted/50 p-2 text-xs" role="status">
          <p>{outcome.message}</p>
          {outcome.detail && <p className="select-text break-all font-mono text-[11px]">{outcome.detail}</p>}
          {outcome.pairing_code && (
            <p>
              Codice di abbinamento: <span className="select-text font-mono font-semibold">{outcome.pairing_code}</span>
              <span className="block text-[11px] text-muted-foreground">{PAIRING_CODE_NOTE}</span>
            </p>
          )}
        </div>
      )}
    </div>
  );
}
