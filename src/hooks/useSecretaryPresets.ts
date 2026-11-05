import { useEffect, useState } from "react";
import { api } from "../lib/tauri";
import type { HubSecretaryPreset } from "../lib/tauri";
import { errorMessage } from "../lib/errors";

/** I segretari predefiniti, letti una volta dal backend (i testi non si duplicano nel frontend). */
export function useSecretaryPresets(): { presets: HubSecretaryPreset[]; error: string | null } {
  const [presets, setPresets] = useState<HubSecretaryPreset[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    api
      .hubListSecretaryPresets()
      .then((list) => {
        if (active) setPresets(list);
      })
      .catch((e: unknown) => {
        if (active) setError(errorMessage(e));
      });
    return () => {
      active = false;
    };
  }, []);

  return { presets, error };
}
