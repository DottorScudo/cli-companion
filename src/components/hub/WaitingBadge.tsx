import { cn } from "../../lib/utils";

interface Props {
  count: number;
  className?: string;
}

/** Pillola arancione col numero di thread che aspettano l'utente; il testo accessibile lo dà chi la contiene. */
export default function WaitingBadge({ count, className }: Props) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        "flex h-4 min-w-4 flex-shrink-0 items-center justify-center rounded-full bg-status-attention px-1 text-[10.5px] font-semibold text-background",
        className,
      )}
    >
      {count}
    </span>
  );
}
