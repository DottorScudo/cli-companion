import { X } from "lucide-react";
import type { HubSecretaryPreset } from "../../../lib/tauri";
import { Button } from "../../ui/button";

interface Props {
  preset: HubSecretaryPreset;
  disabled: boolean;
  /** Apre l'editor precompilato dal modello, come «Aggiungi da modello». */
  onCreate: (preset: HubSecretaryPreset) => void;
  onDismiss: () => void;
}

/** Suggerimento discreto a creare l'Operatore, che non è tra i segretari predefiniti. */
export default function OperatorHint({ preset, disabled, onCreate, onDismiss }: Props) {
  return (
    <aside
      aria-label="Suggerimento: Operatore"
      className="flex flex-wrap items-center gap-2 rounded-md border border-dashed bg-background/40 px-3 py-2"
    >
      <p className="min-w-0 flex-1 text-xs">
        Vuoi un segretario che gestisca la coda?{" "}
        <span className="text-muted-foreground">
          {preset.label} (@{preset.name}): {preset.description}
        </span>
      </p>
      <Button type="button" size="sm" variant="outline" disabled={disabled} onClick={() => onCreate(preset)}>
        Crea {preset.label}
      </Button>
      <button
        type="button"
        onClick={onDismiss}
        aria-label="Chiudi il suggerimento"
        title="Chiudi il suggerimento"
        className="flex-shrink-0 rounded p-0.5 text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
      >
        <X className="!size-3.5" aria-hidden="true" />
      </button>
    </aside>
  );
}
