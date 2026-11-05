import { useStore } from "../../../hooks/useStore";
import { numberRangeHint } from "../../../lib/connectorForm";
import type { FieldValue, ThreadRef } from "../../../lib/connectorForm";
import { GENERAL_CHANNEL } from "../../../lib/hub";
import type { ConnectorConfigField } from "../../../lib/tauri";
import { visibleWorkspaces, workspacesStore } from "../../../stores/workspaces";
import { Input } from "../../ui/input";
import { selectClass } from "../shared";
import ThreadPicker from "./ThreadPicker";

interface Props {
  field: ConnectorConfigField;
  value: FieldValue | undefined;
  onChange: (value: FieldValue) => void;
  /** L'errore del campo (locale o del backend). */
  error?: string;
  disabled?: boolean;
}

const asText = (value: FieldValue | undefined): string => (typeof value === "string" ? value : "");

const asThread = (value: FieldValue | undefined): ThreadRef =>
  typeof value === "object" && value !== null ? value : { channel: GENERAL_CHANNEL, thread_id: "" };

/** Un campo del form dai dati del descrittore: il controllo dipende solo dal tipo del campo. */
export default function FieldControl({ field, value, onChange, error, disabled }: Props) {
  const workspaces = useStore(workspacesStore, visibleWorkspaces);
  const kind = field.kind;
  const id = `connector-field-${field.key}`;
  const help = [field.help, numberRangeHint(field)].filter((part) => part).join(" · ");
  const label = field.required ? `${field.label} *` : field.label;

  const control = () => {
    switch (kind.kind) {
      case "bool":
        return (
          <label className="flex items-center gap-2 text-[13px]">
            <input
              type="checkbox"
              checked={value === true}
              disabled={disabled}
              onChange={(e) => onChange(e.target.checked)}
              className="h-3.5 w-3.5 accent-primary"
            />
            {field.label}
          </label>
        );
      case "select":
        return (
          <select
            id={id}
            value={asText(value)}
            disabled={disabled}
            onChange={(e) => onChange(e.target.value)}
            className={selectClass}
          >
            {!field.required && <option value="">—</option>}
            {field.required && asText(value) === "" && <option value="">Scegli…</option>}
            {kind.options.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        );
      case "workspace":
        return (
          <select
            id={id}
            value={asText(value)}
            disabled={disabled}
            onChange={(e) => onChange(e.target.value)}
            className={selectClass}
          >
            <option value="">{workspaces.length === 0 ? "Nessun workspace" : "Scegli un workspace"}</option>
            {workspaces.map((w) => (
              <option key={w.id} value={w.id}>
                {w.name}
              </option>
            ))}
          </select>
        );
      case "thread": {
        const ref = asThread(value);
        return (
          <ThreadPicker
            label={field.label}
            channel={ref.channel}
            threadId={ref.thread_id}
            disabled={disabled}
            onChange={(channel, threadId) => onChange({ channel, thread_id: threadId })}
          />
        );
      }
      case "number":
        return (
          <Input
            id={id}
            type="number"
            inputMode="numeric"
            min={kind.min}
            max={kind.max}
            step={1}
            value={asText(value)}
            disabled={disabled}
            onChange={(e) => onChange(e.target.value)}
          />
        );
      default:
        return (
          <Input
            id={id}
            value={asText(value)}
            maxLength={kind.kind === "text" ? kind.max_len : undefined}
            placeholder={kind.kind === "text" ? kind.placeholder : undefined}
            disabled={disabled}
            onChange={(e) => onChange(e.target.value)}
          />
        );
    }
  };

  return (
    <div>
      {kind.kind !== "bool" && (
        <label htmlFor={id} className="mb-1 block text-xs font-semibold">
          {label}
        </label>
      )}
      {control()}
      {help && <p className="mt-1 text-[11px] text-muted-foreground">{help}</p>}
      {error && (
        <p className="mt-1 text-[11px] text-destructive" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
