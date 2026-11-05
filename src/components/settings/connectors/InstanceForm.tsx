import { useState } from "react";
import { createInstance, updateInstance } from "../../../app/actions/connectors";
import { initialValues, splitBackendError, toPayload, validateValues } from "../../../lib/connectorForm";
import type { FieldErrors, FieldValue, FormValues } from "../../../lib/connectorForm";
import { LABEL_MAX_CHARS } from "../../../lib/connectors";
import type { ConnectorDescriptor, ConnectorInstanceView } from "../../../lib/tauri";
import { Button } from "../../ui/button";
import { Input } from "../../ui/input";
import FieldsForm from "./FieldsForm";
import { useOperation } from "./useOperation";

interface Props {
  descriptor: ConnectorDescriptor;
  /** L'istanza da modificare; `null` per crearne una. */
  view: ConnectorInstanceView | null;
  onDone: () => void;
  onCancel: () => void;
}

/** Crea o modifica un'istanza: etichetta e campi del descrittore. I segreti si impostano dopo la creazione. */
export default function InstanceForm({ descriptor, view, onDone, onCancel }: Props) {
  const [label, setLabel] = useState(view?.instance.label ?? descriptor.label);
  const [values, setValues] = useState<FormValues>(() =>
    initialValues(descriptor.config_fields, view?.instance.config ?? {}),
  );
  const [errors, setErrors] = useState<FieldErrors>({});
  const [labelError, setLabelError] = useState<string | null>(null);
  const { run, busy, error, setError } = useOperation();

  const change = (key: string, value: FieldValue) => {
    setValues((current) => ({ ...current, [key]: value }));
    setErrors(({ [key]: _cleared, ...rest }) => rest);
    setError(null);
  };

  const submit = async () => {
    const trimmed = label.trim();
    const local = validateValues(descriptor.config_fields, values);
    const tooLong = Array.from(trimmed).length > LABEL_MAX_CHARS;
    setLabelError(trimmed === "" ? "Obbligatoria." : tooLong ? `Al massimo ${LABEL_MAX_CHARS} caratteri.` : null);
    setErrors(local);
    if (trimmed === "" || tooLong || Object.keys(local).length > 0) return;
    const config = toPayload(descriptor.config_fields, values);
    const ok = await run(() =>
      view === null
        ? createInstance(descriptor.id, trimmed, config)
        : updateInstance(view.instance.id, trimmed, config),
    );
    if (ok) onDone();
  };

  const remote = error === null ? null : splitBackendError(error, descriptor.config_fields);
  const shownErrors = remote?.field ? { ...errors, [remote.field]: remote.text } : errors;
  const generalError = remote !== null && remote.field === null ? remote.text : null;

  return (
    <form
      className="flex flex-col gap-3 rounded-md border bg-background/50 p-3"
      onSubmit={(e) => {
        e.preventDefault();
        void submit();
      }}
    >
      <div>
        <label htmlFor="connector-instance-label" className="mb-1 block text-xs font-semibold">
          Etichetta *
        </label>
        <Input
          id="connector-instance-label"
          value={label}
          maxLength={LABEL_MAX_CHARS}
          disabled={busy}
          onChange={(e) => {
            setLabel(e.target.value);
            setError(null);
          }}
        />
        {labelError && (
          <p className="mt-1 text-[11px] text-destructive" role="alert">
            {labelError}
          </p>
        )}
      </div>
      <FieldsForm
        fields={descriptor.config_fields}
        values={values}
        errors={shownErrors}
        onChange={change}
        disabled={busy}
      />
      {generalError && (
        <p className="text-xs text-destructive" role="alert">
          {generalError}
        </p>
      )}
      <div className="flex items-center gap-2">
        <Button type="submit" size="sm" disabled={busy}>
          {view === null ? "Aggiungi" : "Salva"}
        </Button>
        <Button type="button" size="sm" variant="secondary" disabled={busy} onClick={onCancel}>
          Annulla
        </Button>
      </div>
    </form>
  );
}
