import type { RowTaskCounts } from "./useWorkspaceRow";

/** Contatori del pulsante Tasks: in corso (ambra) · in coda (attenuato); niente se entrambi a zero. */
export default function TaskCountsLabel({ counts }: { counts: RowTaskCounts | undefined }) {
  const running = counts?.running ?? 0;
  const queued = counts?.queued ?? 0;
  if (running === 0 && queued === 0) return null;
  return (
    <span
      className="ml-auto flex items-center gap-1 font-mono text-[11px]"
      title={`${running} in corso · ${queued} in coda`}
    >
      <span className={running > 0 ? "text-status-running" : "text-faint"}>{running}</span>
      <span className="text-faint" aria-hidden="true">
        ·
      </span>
      <span className="text-muted-foreground">{queued}</span>
    </span>
  );
}
