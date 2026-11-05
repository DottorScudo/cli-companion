import { useState } from "react";
import { clearSecret, setSecret } from "../../../app/actions/connectors";
import { secretFields } from "../../../lib/connectorForm";
import { isSecretSet } from "../../../lib/connectors";
import type { ConnectorConfigField, ConnectorInstanceView } from "../../../lib/tauri";
import { Button } from "../../ui/button";
import { Input } from "../../ui/input";
import { useOperation } from "./useOperation";

/** Un segreto in sola scrittura: si vede solo se è impostato; il valore scritto non torna mai. */
function SecretRow({ view, field }: { view: ConnectorInstanceView; field: ConnectorConfigField }) {
  const [value, setValue] = useState("");
  const { run, busy, error } = useOperation();
  const id = view.instance.id;
  const isSet = isSecretSet(view, field.key);

  const save = async () => {
    if (await run(() => setSecret(id, field.key, value))) setValue("");
  };

  return (
    <div>
      <div className="mb-1 flex items-center justify-between gap-2">
        <label htmlFor={`secret-${id}-${field.key}`} className="text-xs font-semibold">
          {field.required ? `${field.label} *` : field.label}
        </label>
        <span className="text-[11px] text-muted-foreground">{isSet ? "Impostato" : "Non impostato"}</span>
      </div>
      <div className="flex gap-2">
        <Input
          id={`secret-${id}-${field.key}`}
          type="password"
          autoComplete="off"
          spellCheck={false}
          value={value}
          placeholder={isSet ? "Scrivi un nuovo valore per sostituirlo" : "Scrivi il valore"}
          disabled={busy}
          onChange={(e) => setValue(e.target.value)}
        />
        <Button size="sm" className="h-9" disabled={busy || value === ""} onClick={() => void save()}>
          {isSet ? "Sostituisci" : "Imposta"}
        </Button>
        {isSet && (
          <Button
            size="sm"
            variant="outline"
            className="h-9"
            disabled={busy}
            onClick={() => void run(() => clearSecret(id, field.key))}
          >
            Togli
          </Button>
        )}
      </div>
      {field.help && <p className="mt-1 text-[11px] text-muted-foreground">{field.help}</p>}
      {error && (
        <p className="mt-1 text-[11px] text-destructive" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}

/** I campi `secret` del descrittore di un'istanza già creata. */
export default function SecretFields({
  view,
  fields,
}: {
  view: ConnectorInstanceView;
  fields: readonly ConnectorConfigField[];
}) {
  const secrets = secretFields(fields);
  if (secrets.length === 0) return null;
  return (
    <div className="flex flex-col gap-3">
      {secrets.map((field) => (
        <SecretRow key={field.key} view={view} field={field} />
      ))}
    </div>
  );
}
