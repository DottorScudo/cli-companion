import { useCallback, useState } from "react";
import { setBotSettings } from "../../../app/actions/settings";
import { errorMessage } from "../../../lib/errors";
import type { BotSettings } from "../../../lib/tauri";

/**
 * Operazione sulle impostazioni dei bot: il backend restituisce quelle salvate,
 * che finiscono nello store; in caso di errore lo stato resta quello di prima
 * e il messaggio è in `error`.
 */
export function useBotOp() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = useCallback(async (op: () => Promise<BotSettings>): Promise<boolean> => {
    setError(null);
    setBusy(true);
    try {
      setBotSettings(await op());
      return true;
    } catch (e) {
      setError(errorMessage(e));
      return false;
    } finally {
      setBusy(false);
    }
  }, []);

  return { busy, error, setError, run };
}
