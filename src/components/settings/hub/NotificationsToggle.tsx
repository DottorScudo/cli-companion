import { useState } from "react";
import { saveNotifications } from "../../../app/actions/hubSettings";
import { errorMessage } from "../../../lib/errors";
import type { HubSettingsView } from "../../../lib/tauri";

/** Interruttore «Notifiche di sistema» dell'Agents HUB: si salva subito; in caso di errore resta com'era. */
export default function NotificationsToggle({ settings }: { settings: HubSettingsView }) {
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const change = async (next: boolean) => {
    setError(null);
    setSaving(true);
    try {
      await saveNotifications(next);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="flex flex-col gap-1 border-t pt-3">
      <label className="flex items-center gap-2 text-[13px]">
        <input
          type="checkbox"
          checked={settings.notifications}
          disabled={saving}
          onChange={(e) => void change(e.target.checked)}
          className="h-3.5 w-3.5 accent-primary"
        />
        Notifiche di sistema
      </label>
      <p className="text-[11px] text-muted-foreground">
        Una notifica di Windows per le risposte dei segretari e per gli eventi dei thread (proposte, task finiti,
        domande dei worker, task falliti o in attenzione), se non stai guardando quel thread. Cliccandola si apre il
        thread. Spente, i task falliti, in attenzione o con una domanda notificano come prima.
      </p>
      {error && <p className="text-[11px] text-destructive">{error}</p>}
    </div>
  );
}
