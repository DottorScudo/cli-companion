import type { HubChannel } from "../../lib/hub";
import { cn } from "../../lib/utils";
import { selectClass } from "../settings/shared";

interface Props {
  label: string;
  channels: readonly HubChannel[];
  value: string;
  onChange: (channel: string) => void;
  /** Voce in cima che vale «nessun canale» (valore vuoto), per i filtri. */
  allLabel?: string;
  disabled?: boolean;
  className?: string;
}

/** Selettore del canale (Generale e workspace visibili). */
export default function ChannelSelect({ label, channels, value, onChange, allLabel, disabled, className }: Props) {
  return (
    <select
      aria-label={label}
      value={value}
      disabled={disabled}
      onChange={(e) => onChange(e.target.value)}
      className={cn(selectClass, "h-7 text-[11px]", className)}
    >
      {allLabel !== undefined && <option value="">{allLabel}</option>}
      {channels.map((c) => (
        <option key={c.id} value={c.id}>
          {c.name}
        </option>
      ))}
    </select>
  );
}
