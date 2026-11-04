import type { HubChannel } from "../../../lib/hub";
import { GENERAL_CHANNEL } from "../../../lib/hub";
import { SECRETARY_TOOLS, toggleChannel, toggleTool } from "../../../lib/hubSettings";
import type { SecretaryDraft } from "../../../lib/hubSettings";

const CHECK_CLASS = "mt-0.5 h-3.5 w-3.5 accent-primary";

interface ToolsProps {
  draft: SecretaryDraft;
  disabled: boolean;
  onChange: (tools: SecretaryDraft["tools"]) => void;
}

/** Tool consentiti: una casella per tool, con la descrizione breve. */
export function ToolsField({ draft, disabled, onChange }: ToolsProps) {
  return (
    <fieldset className="flex flex-col gap-2" disabled={disabled}>
      <legend className="mb-1 text-xs font-semibold">Tool consentiti</legend>
      <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
        {SECRETARY_TOOLS.map((tool) => (
          <label key={tool.id} className="flex items-start gap-2 rounded-md border px-2.5 py-2">
            <input
              type="checkbox"
              checked={draft.tools.includes(tool.id)}
              onChange={() => onChange(toggleTool(draft.tools, tool.id))}
              className={CHECK_CLASS}
            />
            <span className="flex flex-col gap-0.5">
              <span className="font-mono text-xs">{tool.id}</span>
              <span className="text-[11px] text-muted-foreground">{tool.description}</span>
            </span>
          </label>
        ))}
      </div>
    </fieldset>
  );
}

interface ChannelsProps {
  draft: SecretaryDraft;
  channels: readonly HubChannel[];
  disabled: boolean;
  onChange: (channels: SecretaryDraft["channels"]) => void;
}

/** Canali in cui risponde: tutti (anche quelli futuri) o una selezione tra Generale e workspace. */
export function ChannelsField({ draft, channels, disabled, onChange }: ChannelsProps) {
  const all = draft.channels === null;
  const ids = channels.map((c) => c.id);
  return (
    <fieldset className="flex flex-col gap-2" disabled={disabled}>
      <legend className="mb-1 text-xs font-semibold">Canali in cui risponde</legend>
      <label className="flex items-center gap-2 text-[13px]">
        <input type="radio" name="secretary-channels" checked={all} onChange={() => onChange(null)} />
        Tutti i canali, anche quelli futuri
      </label>
      <label className="flex items-center gap-2 text-[13px]">
        <input type="radio" name="secretary-channels" checked={!all} onChange={() => onChange(draft.channels ?? [])} />
        Solo questi:
      </label>
      {!all && (
        <div className="flex flex-wrap gap-2 pl-6">
          {channels.map((c) => (
            <label key={c.id} className="flex items-center gap-1.5 rounded-md border px-2.5 py-1.5 text-[13px]">
              <input
                type="checkbox"
                checked={draft.channels?.includes(c.id) ?? false}
                onChange={() => onChange(toggleChannel(draft.channels ?? [], c.id, ids))}
                className="h-3.5 w-3.5 accent-primary"
              />
              {c.id === GENERAL_CHANNEL ? "Generale" : c.name}
            </label>
          ))}
        </div>
      )}
    </fieldset>
  );
}
