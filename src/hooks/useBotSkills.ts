import { useEffect, useState } from "react";
import { api } from "../lib/tauri";
import type { BotSkillOption } from "../lib/tauri";
import { errorMessage } from "../lib/errors";

/** Le skill selezionabili per un Bot (del plugin, poi dell'utente), lette una volta dal backend. */
export function useBotSkills(): { skills: BotSkillOption[]; error: string | null } {
  const [skills, setSkills] = useState<BotSkillOption[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    api
      .listBotSkills()
      .then((list) => {
        if (active) setSkills(list);
      })
      .catch((e: unknown) => {
        if (active) setError(errorMessage(e));
      });
    return () => {
      active = false;
    };
  }, []);

  return { skills, error };
}
