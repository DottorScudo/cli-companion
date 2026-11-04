import { useEffect, useState } from "react";
import { Loader2 } from "lucide-react";
import { Button } from "../ui/button";
import { api } from "../../lib/tauri";
import { useTauriEvent } from "../../lib/useTauriEvent";
import type { E2eRun } from "../../lib/board";
import { afterRunResponse } from "../../lib/e2e";
import E2eResult from "../tasks/E2eResult";
import OpenE2eReportButton from "../tasks/OpenE2eReportButton";

interface Props {
  workspaceId: string;
  repo: string;
}

/** Esecuzione a mano degli E2E di un repository: pulsante e ultimo esito, aggiornato dall'evento `e2e-update`. */
export default function RepoE2eRun({ workspaceId, repo }: Props) {
  const [last, setLast] = useState<E2eRun | null>(null);
  const [error, setError] = useState<string | null>(null);

  useTauriEvent("e2e-update", (e) => {
    if (e.workspace_id === workspaceId && e.repo === repo) setLast(e.run);
  });

  useEffect(() => {
    let cancelled = false;
    setLast(null);
    setError(null);
    api
      .e2eLastResult(workspaceId, repo)
      .then((r) => !cancelled && setLast(r))
      .catch((e) => !cancelled && setError(String(e)));
    return () => {
      cancelled = true;
    };
  }, [workspaceId, repo]);

  const running = last?.status === "running";
  const start = () => {
    setError(null);
    api
      .e2eRunRepo(workspaceId, repo)
      .then((r) => setLast((prev) => afterRunResponse(prev, r)))
      .catch((e) => setError(String(e)));
  };

  return (
    <div className="flex flex-col gap-2">
      <div>
        <Button size="sm" variant="secondary" disabled={running} onClick={start}>
          {running && <Loader2 className="mr-1.5 h-3.5 w-3.5 animate-spin" />}
          Esegui E2E
        </Button>
      </div>
      {error && <p className="text-[11px] text-destructive">{error}</p>}
      {last && <E2eResult run={last} />}
      {last && <OpenE2eReportButton workspaceId={workspaceId} repo={repo} disabled={running} />}
    </div>
  );
}
