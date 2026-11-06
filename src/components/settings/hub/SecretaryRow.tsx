import { useState } from "react";
import { allowsChannel, isConfigured, promptFirstLine } from "../../../lib/hubSecretaries";
import { PRESET_UPDATE_NOTICE } from "../../../lib/hubPresets";
import { SECRETARY_TOOLS } from "../../../lib/hubSettings";
import type { HubChannel } from "../../../lib/hub";
import type { HubSecretary } from "../../../lib/tauri";
import { cn } from "../../../lib/utils";
import BotAvatar from "../../BotAvatar";
import { Button } from "../../ui/button";

const AVATAR_SIZE = 32;

/** «tutti i canali», «Generale», «Generale, sito-web». */
function channelsText(secretary: HubSecretary, channels: readonly HubChannel[]): string {
  if (secretary.channels.mode === "all") return "tutti i canali";
  const names = channels.filter((c) => allowsChannel(secretary, c.id)).map((c) => c.name);
  return names.length === 0 ? "nessun canale" : names.join(", ");
}

interface Props {
  secretary: HubSecretary;
  channels: readonly HubChannel[];
  /** Il modello da cui nasce ha ora un prompt diverso da quello salvato. */
  updated: boolean;
  disabled: boolean;
  onEdit: () => void;
  onDelete: () => void;
}

/** Riga chiusa di un segretario: avatar, `@nome`, prima riga del prompt, tool, canali e l'avviso di un modello aggiornato. */
export default function SecretaryRow({ secretary, channels, updated, disabled, onEdit, onDelete }: Props) {
  const [confirming, setConfirming] = useState(false);
  const configured = isConfigured(secretary);
  const tools = SECRETARY_TOOLS.filter((t) => secretary.tools.includes(t.id));

  return (
    <li
      className={cn("flex items-center gap-3 rounded-lg border px-3 py-2.5", !configured && "border-dashed")}
      aria-label={`@${secretary.name}`}
    >
      <BotAvatar
        name={secretary.name}
        size={AVATAR_SIZE}
        image={secretary.image ?? undefined}
        className={configured ? undefined : "opacity-55"}
      />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <div className="flex items-baseline gap-2">
          <span className="font-mono text-[13px] font-semibold">@{secretary.name}</span>
          <span className="min-w-0 truncate text-[11px] text-text-secondary">
            {configured ? promptFirstLine(secretary.system_prompt) : "Non configurato: non risponde alle menzioni"}
          </span>
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          {tools.map((t) => (
            <span key={t.id} className="rounded bg-surface px-1.5 py-px font-mono text-[10.5px] text-text-secondary">
              {t.id}
            </span>
          ))}
          <span className="text-[11px] text-muted-foreground">
            {tools.length > 0 && "· "}
            {channelsText(secretary, channels)}
          </span>
        </div>
        {updated && <span className="text-[11px] text-primary">{PRESET_UPDATE_NOTICE}</span>}
      </div>
      {confirming ? (
        <div className="flex flex-shrink-0 items-center gap-2">
          <span className="text-[11px] text-muted-foreground">Eliminare @{secretary.name}?</span>
          <Button type="button" size="sm" variant="destructive" disabled={disabled} onClick={onDelete}>
            Elimina
          </Button>
          <Button type="button" size="sm" variant="outline" onClick={() => setConfirming(false)}>
            Annulla
          </Button>
        </div>
      ) : (
        <div className="flex flex-shrink-0 gap-2">
          <Button type="button" size="sm" variant="outline" disabled={disabled} onClick={onEdit}>
            {configured ? "Modifica" : "Configura"}
          </Button>
          <Button type="button" size="sm" variant="outline" disabled={disabled} onClick={() => setConfirming(true)}>
            Elimina
          </Button>
        </div>
      )}
    </li>
  );
}
