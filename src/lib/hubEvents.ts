// Resa dei messaggi evento dell'Agents HUB: etichetta, icona e tono per tipo.
// Il backend può aggiungere tipi (compatibile in lettura): uno sconosciuto si
// mostra con la resa generica, mai come errore.

/** Chiave dell'icona; il componente la traduce in un'icona. */
export type HubEventIcon = "proposal" | "started" | "done" | "failed" | "attention" | "deleted" | "generic";

export type HubEventTone = "neutral" | "success" | "danger" | "warning";

export interface HubEventLook {
  label: string;
  icon: HubEventIcon;
  tone: HubEventTone;
}

const GENERIC_LOOK: HubEventLook = { label: "Evento", icon: "generic", tone: "neutral" };

const EVENT_LOOKS: Readonly<Record<string, HubEventLook>> = {
  proposal_created: { label: "Proposta", icon: "proposal", tone: "warning" },
  proposal_accepted: { label: "Proposta accettata", icon: "proposal", tone: "success" },
  proposal_rejected: { label: "Proposta rifiutata", icon: "proposal", tone: "neutral" },
  task_started: { label: "Avviato", icon: "started", tone: "neutral" },
  task_done: { label: "Completato", icon: "done", tone: "success" },
  task_failed: { label: "Fallito", icon: "failed", tone: "danger" },
  task_attention: { label: "Attenzione", icon: "attention", tone: "warning" },
  task_deleted: { label: "Eliminato", icon: "deleted", tone: "neutral" },
};

/** Resa di un tipo di evento (anche uno che questa versione non conosce). */
export function eventLook(kind: string): HubEventLook {
  return EVENT_LOOKS[kind] ?? GENERIC_LOOK;
}
