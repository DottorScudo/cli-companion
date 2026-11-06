import { useState } from "react";
import { restoreSecretaryPreset } from "../../../app/actions/hubSettings";
import { errorMessage } from "../../../lib/errors";
import { PRESET_UPDATE_NOTICE } from "../../../lib/hubPresets";
import type { HubSecretaryPreset } from "../../../lib/tauri";
import { Button } from "../../ui/button";

interface Props {
  /** Nome del segretario da ripristinare. */
  name: string;
  preset: HubSecretaryPreset;
  /** Il prompt del modello è cambiato rispetto a quello salvato: lo si segnala accanto al pulsante. */
  updated: boolean;
  disabled: boolean;
  onRestored: () => void;
  onError: (message: string) => void;
}

/** «Ripristina predefinito»: riporta prompt e tool al testo del modello, dopo conferma. Nome, canali e immagine restano. */
export default function PresetRestore({ name, preset, updated, disabled, onRestored, onError }: Props) {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);

  const restore = async () => {
    setBusy(true);
    try {
      await restoreSecretaryPreset(name);
      onRestored();
    } catch (e) {
      onError(errorMessage(e));
      setBusy(false);
      setConfirming(false);
    }
  };

  if (!confirming) {
    return (
      <div className="flex flex-wrap items-center gap-2">
        <Button type="button" size="sm" variant="outline" disabled={disabled} onClick={() => setConfirming(true)}>
          Ripristina predefinito
        </Button>
        {updated && <span className="text-[11px] text-primary">{PRESET_UPDATE_NOTICE}</span>}
      </div>
    );
  }
  return (
    <div className="flex flex-wrap items-center gap-2">
      <span className="text-[11px] text-muted-foreground">
        Prompt e tool tornano quelli del modello {preset.label}; le modifiche non salvate nell’editor si perdono.
      </span>
      <Button type="button" size="sm" variant="destructive" disabled={busy || disabled} onClick={() => void restore()}>
        Ripristina
      </Button>
      <Button type="button" size="sm" variant="outline" disabled={busy} onClick={() => setConfirming(false)}>
        Annulla
      </Button>
    </div>
  );
}
