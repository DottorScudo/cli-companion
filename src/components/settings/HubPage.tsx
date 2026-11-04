import { useState } from "react";
import { setHubEnabled } from "../../app/actions/hub";
import { errorMessage } from "../../lib/errors";
import { useStore } from "../../hooks/useStore";
import { hubStore } from "../../stores/hub";
import { Section, SettingsPage } from "./shared";

/** Generale › Agents HUB: l'interruttore della modalità (spenta di default). */
export default function HubPage() {
  const enabled = useStore(hubStore, (s) => s.enabled);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  /** Salva subito; in caso di errore l'interruttore resta com'era. */
  const change = async (next: boolean) => {
    setError(null);
    setSaving(true);
    try {
      await setHubEnabled(next);
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setSaving(false);
    }
  };

  return (
    <SettingsPage
      title="Agents HUB"
      description="Uno spazio stile chat con un canale per workspace e un canale Generale, dove seguire i thread e gli eventi dei task."
    >
      <Section>
        <label className="flex items-center gap-2 text-[13px]">
          <input
            type="checkbox"
            checked={enabled}
            disabled={saving}
            onChange={(e) => void change(e.target.checked)}
            className="h-3.5 w-3.5 accent-primary"
          />
          Attiva Agents HUB
        </label>
        <p className="-mt-1 text-[11px] text-muted-foreground">
          Modalità opzionale: spenta, l’app resta com’è. Accesa, compare un pulsante nella sidebar che apre la vista; i
          thread già scritti restano salvati anche se la spegni.
        </p>
        {error && <p className="text-[11px] text-destructive">{error}</p>}
      </Section>
    </SettingsPage>
  );
}
