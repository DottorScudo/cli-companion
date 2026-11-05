import { useCallback, useState } from "react";
import { errorMessage } from "../../../lib/errors";

/** Esegue un'operazione del backend tenendo traccia di «in corso» ed errore; dice se è riuscita. */
export function useOperation() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const run = useCallback(async (operation: () => Promise<unknown>): Promise<boolean> => {
    setError(null);
    setBusy(true);
    try {
      await operation();
      return true;
    } catch (e) {
      setError(errorMessage(e));
      return false;
    } finally {
      setBusy(false);
    }
  }, []);
  return { run, busy, error, setError };
}
