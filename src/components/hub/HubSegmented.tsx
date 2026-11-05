import { cn } from "../../lib/utils";

interface Option<T extends string> {
  id: T;
  label: string;
}

interface Props<T extends string> {
  label: string;
  options: readonly Option<T>[];
  value: T;
  onChange: (value: T) => void;
  className?: string;
}

/** Interruttore segmentato (come il filtro Aperti/Tutti): un pulsante per opzione, `aria-pressed` su quella scelta. */
export default function HubSegmented<T extends string>({ label, options, value, onChange, className }: Props<T>) {
  return (
    <div role="group" aria-label={label} className={cn("flex gap-1", className)}>
      {options.map((o) => (
        <button
          key={o.id}
          type="button"
          aria-pressed={value === o.id}
          onClick={() => onChange(o.id)}
          className={cn(
            "rounded px-2 py-0.5 text-[11px] transition-colors",
            value === o.id ? "bg-accent text-foreground" : "text-muted-foreground hover:text-foreground",
          )}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}
