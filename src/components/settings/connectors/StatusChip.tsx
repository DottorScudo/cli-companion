import { useManagerNow } from "../../tasks/useManagerNow";
import { statusView } from "../../../lib/connectors";
import type { StatusTone } from "../../../lib/connectors";
import type { ConnectorStatus } from "../../../lib/tauri";
import { cn } from "../../../lib/utils";

const TONE_CLASS: Record<StatusTone, string> = {
  ok: "border-emerald-500/40 bg-emerald-500/10",
  warn: "border-amber-500/40 bg-amber-500/10",
  error: "border-destructive/40 bg-destructive/10 text-destructive",
  muted: "text-muted-foreground",
};

/** Lo stato di salute di un'istanza: il chip e, sotto, il messaggio o il prossimo tentativo. */
export default function StatusChip({ status }: { status: ConnectorStatus }) {
  const now = useManagerNow();
  const view = statusView(status, now);
  return (
    <div className="flex min-w-0 flex-col items-end gap-0.5" role="status">
      <span className={cn("rounded border px-1.5 py-px text-[10px] font-medium", TONE_CLASS[view.tone])}>
        {view.label}
      </span>
      {view.detail && (
        <span className="max-w-full break-words text-right text-[11px] text-muted-foreground">{view.detail}</span>
      )}
    </div>
  );
}
