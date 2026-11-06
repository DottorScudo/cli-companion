import type { BotLaunch, BuiltinLaunch } from "../../lib/tauri";

interface Props {
  builtins: readonly BuiltinLaunch[];
  bots: readonly BotLaunch[];
}

/** Le opzioni condivise di un avvio predefinito: gli integrati, poi i Bot in un gruppo. */
export default function DefaultLaunchOptions({ builtins, bots }: Props) {
  return (
    <>
      {builtins.map((b) => (
        <option key={b.id} value={b.id}>
          {b.name}
        </option>
      ))}
      {bots.length > 0 && (
        <optgroup label="Bot">
          {bots.map((b) => (
            <option key={b.id} value={b.id}>
              {b.name}
            </option>
          ))}
        </optgroup>
      )}
    </>
  );
}
