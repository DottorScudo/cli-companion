import { cn } from "../lib/utils";

/** Pallino "aggiornamento disponibile" accanto al pulsante delle Impostazioni. */
export default function UpdateDot({ className }: { className?: string }) {
  return (
    <span
      className={cn("h-1.5 w-1.5 rounded-full bg-primary", className)}
      title="Aggiornamento disponibile"
      role="img"
      aria-label="Aggiornamento disponibile"
    />
  );
}
