import { useState } from "react";
import { deleteRoute, updateRoute } from "../../../app/actions/connectors";
import { routeInputWithEnabled, wakeLabel } from "../../../lib/connectorRoutes";
import type { ConnectorRoute } from "../../../lib/tauri";
import { Button } from "../../ui/button";
import { useOperation } from "./useOperation";

interface Props {
  route: ConnectorRoute;
  /** Una riga in più davanti al risveglio: dove va a finire la regola, o di quale istanza è. */
  context?: string;
  onEdit?: () => void;
}

/** Una regola in una riga: etichetta, destinazione, risveglio, abilitazione, modifica ed eliminazione con conferma. */
export default function RouteRow({ route, context, onEdit }: Props) {
  const [confirming, setConfirming] = useState(false);
  const { run, busy, error } = useOperation();

  return (
    <li className="flex flex-col gap-1.5 rounded-md border p-2">
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1">
        <span className="min-w-0 flex-1 truncate text-[13px] font-medium">{route.label}</span>
        <label className="flex items-center gap-1.5 text-xs">
          <input
            type="checkbox"
            checked={route.enabled}
            disabled={busy}
            onChange={(e) => void run(() => updateRoute(route.id, routeInputWithEnabled(route, e.target.checked)))}
            className="h-3.5 w-3.5 accent-primary"
          />
          Abilitata
        </label>
        {onEdit && (
          <Button size="sm" variant="outline" className="h-7 px-2" disabled={busy} onClick={onEdit}>
            Modifica
          </Button>
        )}
        <Button size="sm" variant="outline" className="h-7 px-2" disabled={busy} onClick={() => setConfirming(true)}>
          Togli
        </Button>
      </div>
      <p className="text-[11px] text-muted-foreground">
        {[context, wakeLabel(route.wake)].filter(Boolean).join(" · ")}
      </p>
      {route.error && <p className="text-[11px] text-destructive">{route.error}</p>}
      {confirming && (
        <div className="flex flex-wrap items-center gap-2 rounded-md border border-destructive/40 p-2" role="alert">
          <p className="flex-1 text-xs">Togliere la regola «{route.label}»?</p>
          <Button
            size="sm"
            variant="destructive"
            className="h-7 px-2"
            disabled={busy}
            onClick={() => void run(() => deleteRoute(route)).then(() => setConfirming(false))}
          >
            Togli
          </Button>
          <Button size="sm" variant="secondary" className="h-7 px-2" onClick={() => setConfirming(false)}>
            Annulla
          </Button>
        </div>
      )}
      {error && (
        <p className="whitespace-pre-wrap text-xs text-destructive" role="alert">
          {error}
        </p>
      )}
    </li>
  );
}
