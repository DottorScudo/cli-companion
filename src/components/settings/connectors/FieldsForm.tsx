import { valueFields } from "../../../lib/connectorForm";
import type { FieldErrors, FieldValue, FormValues } from "../../../lib/connectorForm";
import type { ConnectorConfigField } from "../../../lib/tauri";
import { fieldGridClass } from "../shared";
import FieldControl from "./FieldControl";

interface Props {
  fields: readonly ConnectorConfigField[];
  values: FormValues;
  errors: FieldErrors;
  onChange: (key: string, value: FieldValue) => void;
  disabled?: boolean;
}

/** I campi di un descrittore (configurazione o filtro) come form; i segreti non ci sono: hanno il loro editor. */
export default function FieldsForm({ fields, values, errors, onChange, disabled }: Props) {
  return (
    <div className={fieldGridClass}>
      {valueFields(fields).map((field) => (
        <FieldControl
          key={field.key}
          field={field}
          value={values[field.key]}
          error={errors[field.key]}
          disabled={disabled}
          onChange={(value) => onChange(field.key, value)}
        />
      ))}
    </div>
  );
}
