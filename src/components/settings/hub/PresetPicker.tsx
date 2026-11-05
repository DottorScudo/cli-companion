import type { HubSecretaryPreset } from "../../../lib/tauri";
import { selectClass } from "../shared";

interface Props {
  presets: readonly HubSecretaryPreset[];
  disabled: boolean;
  onPick: (preset: HubSecretaryPreset) => void;
}

/** «Aggiungi da modello»: sceglie uno dei segretari predefiniti, di cui l'editor si apre precompilato. */
export default function PresetPicker({ presets, disabled, onPick }: Props) {
  return (
    <select
      aria-label="Aggiungi da modello"
      value=""
      disabled={disabled || presets.length === 0}
      onChange={(e) => {
        const preset = presets.find((p) => p.id === e.target.value);
        if (preset) onPick(preset);
      }}
      className={`${selectClass} h-8 w-auto min-w-[12rem]`}
    >
      <option value="">Aggiungi da modello…</option>
      {presets.map((p) => (
        <option key={p.id} value={p.id}>
          {p.label} (@{p.name}): {p.description}
        </option>
      ))}
    </select>
  );
}
