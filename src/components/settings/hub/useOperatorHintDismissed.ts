import { useCallback, useState } from "react";
import { browserStorage } from "../../../lib/browserStorage";
import { readOperatorHintDismissed, writeOperatorHintDismissed } from "../../../lib/hubOperatorHint";

/** Il suggerimento dell'Operatore è stato chiuso: letto e ricordato in localStorage. */
export function useOperatorHintDismissed() {
  const [dismissed, setDismissed] = useState(() => readOperatorHintDismissed(browserStorage()));

  const dismiss = useCallback(() => {
    writeOperatorHintDismissed(browserStorage());
    setDismissed(true);
  }, []);

  return { dismissed, dismiss };
}
