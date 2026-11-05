import { useEffect } from "react";
import { reloadRoutes } from "../../app/actions/connectors";
import { useStore } from "../../hooks/useStore";
import { KIND_SECTIONS, descriptorsOfKind, orphanInstances } from "../../lib/connectors";
import { connectorsStore } from "../../stores/connectors";
import { hubStore } from "../../stores/hub";
import DescriptorGroup from "./connectors/DescriptorGroup";
import InstanceCard from "./connectors/InstanceCard";
import { Section, SettingsPage } from "./shared";

/**
 * Generale › Connettori: i connettori compilati nell'app divisi in Fonti e Canali di
 * contatto, costruiti dai descrittori. Si configura anche con l'HUB spento; girano solo a HUB acceso.
 */
export default function ConnectorsPage() {
  const descriptors = useStore(connectorsStore, (s) => s.descriptors);
  const instances = useStore(connectorsStore, (s) => s.instances);
  const loaded = useStore(connectorsStore, (s) => s.loaded);
  const hubEnabled = useStore(hubStore, (s) => s.enabled);

  // Una regola si disabilita da sola se il suo thread sparisce: all'apertura si rileggono.
  useEffect(() => {
    void reloadRoutes();
  }, []);

  const orphans = orphanInstances(instances, descriptors);
  const nothing = loaded && descriptors.length === 0 && instances.length === 0;

  return (
    <SettingsPage
      title="Connettori"
      description="Estendono l’Agents HUB verso l’esterno: le fonti portano dati nei thread, i canali di contatto ti raggiungono fuori dall’app."
    >
      {!hubEnabled && (
        <p className="rounded-md border border-amber-500/40 bg-amber-500/10 p-2 text-xs" role="note">
          L’Agents HUB è spento: puoi configurare i connettori, ma girano solo a HUB acceso.
        </p>
      )}
      {!loaded && <p className="text-xs text-muted-foreground">Caricamento…</p>}
      {nothing && (
        <p className="text-xs text-muted-foreground">Nessun connettore disponibile in questa versione dell’app.</p>
      )}
      {loaded &&
        !nothing &&
        KIND_SECTIONS.map((section) => {
          const ofKind = descriptorsOfKind(descriptors, section.kind);
          return (
            <Section key={section.kind}>
              <div>
                <h2 className="text-[13px] font-semibold">{section.title}</h2>
                <p className="text-[11px] text-muted-foreground">{section.description}</p>
              </div>
              {ofKind.length === 0 && <p className="text-xs text-muted-foreground">{section.empty}</p>}
              {ofKind.map((descriptor) => (
                <DescriptorGroup key={descriptor.id} descriptor={descriptor} instances={instances} />
              ))}
            </Section>
          );
        })}
      {orphans.length > 0 && (
        <Section>
          <h2 className="text-[13px] font-semibold">Non disponibili</h2>
          {orphans.map((view) => (
            <InstanceCard key={view.instance.id} view={view} descriptor={undefined} />
          ))}
        </Section>
      )}
    </SettingsPage>
  );
}
