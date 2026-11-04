/** Avviso: il predefinito salvato punta a un'integrazione non più abilitata (id conservato, parte una shell). */
export default function UnavailableLaunchNotice({ launchId }: { launchId: string | null }) {
  if (launchId === null) return null;
  return (
    <p role="status" className="text-[11px] text-destructive">
      L’avvio predefinito salvato (<code className="font-mono">{launchId}</code>) non è disponibile: l’integrazione è
      disabilitata o non esiste più. Finché non scegli un altro avvio, o la riabiliti, parte una shell vuota.
    </p>
  );
}
