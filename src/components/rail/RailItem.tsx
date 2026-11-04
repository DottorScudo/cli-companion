import type { LucideIcon } from "lucide-react";
import { cn } from "../../lib/utils";
import WaitingBadge from "../hub/WaitingBadge";

interface Props {
  icon: LucideIcon;
  label: string;
  title: string;
  /** Sostituisce l'etichetta come nome accessibile (es. col numero di thread in attesa). */
  ariaLabel?: string;
  active: boolean;
  /** Thread in attesa dell'utente; nascosto a zero. */
  badge?: number;
  onClick: () => void;
}

/** Voce della barra dei mondi: icona sopra, etichetta sotto; la voce attiva è `aria-current`. */
export default function RailItem({ icon: Icon, label, title, ariaLabel, active, badge = 0, onClick }: Props) {
  return (
    <button
      type="button"
      onClick={onClick}
      title={title}
      aria-label={ariaLabel}
      aria-current={active ? "page" : undefined}
      className={cn(
        "relative flex w-[52px] flex-col items-center gap-0.5 rounded-lg py-[7px] text-[11px] transition-colors",
        active
          ? "bg-primary/15 font-medium text-primary"
          : "text-text-secondary hover:bg-surface-2 hover:text-foreground",
      )}
    >
      <Icon className="!size-[18px]" aria-hidden="true" />
      {label}
      {badge > 0 && <WaitingBadge count={badge} className="absolute right-0 top-0" />}
    </button>
  );
}
