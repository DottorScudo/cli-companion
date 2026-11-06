/** Avviso: il predefinito salvato punta a un integrato o a un Bot non più disponibile (id conservato, parte una shell). */
export default function UnavailableLaunchNotice({ launchId }: { launchId: string | null }) {
  if (launchId === null) return null;
  return (
    <p role="status" className="text-[11px] text-destructive">
      L’avvio predefinito salvato (<code className="font-mono">{launchId}</code>) non è disponibile: l’integrazione o il
      Bot è disattivato o non esiste più. Finché non scegli un altro avvio, o lo riattivi, parte una shell vuota.
    </p>
  );
}
