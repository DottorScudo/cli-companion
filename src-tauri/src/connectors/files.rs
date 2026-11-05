//! I file dei connettori, nella busta di `store/` (migrazioni, backup dei file
//! corrotti, scrittura atomica). Mai nella repository dell'utente, mai segreti:
//! - `<app_data>/connectors.json`: `{ instances, routes, links }`;
//! - `<app_data>/connector_state/<istanza>.json`: lo stato di un'istanza (cursori,
//!   id visti, mappe), un file per istanza.
//!
//! Con i connettori mai configurati non si crea nessun file: un file nasce solo da
//! un salvataggio. La lettura di `connectors.json` e' tollerante: una voce
//! illeggibile (o una regola di un'istanza che non c'e') si esclude e si riporta,
//! le altre si caricano e il file originale va in un backup (`store/`). Una
//! istanza di un connettore sconosciuto NON e' illeggibile: resta com'e' e si
//! risalva intatta. Vedi docs/CONNECTORS.MD §6.

use super::model::{ConnectorInstance, ConnectorLink, ConnectorRoute, ConnectorsFile};
use super::state::InstanceState;
use crate::store::{adopt_envelope, Decoded, JsonFile, Loaded, Migration};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

const FILE_NAME: &str = "connectors.json";
/// Cartella degli stati delle istanze dentro la cartella dati dell'app.
const STATE_DIR: &str = "connector_state";
/// Caratteri dell'id di una voce scartata citati nell'avviso.
const SHOWN_ID_MAX: usize = 40;

/// Catena di migrazioni di `connectors.json`: un passo in coda per ogni cambio di formato.
static STEPS: [Migration; 1] = [adopt_envelope];
/// Catena di migrazioni dei file di stato.
static STATE_STEPS: [Migration; 1] = [adopt_envelope];

/// Descrizione del file: i test ne fanno istanze locali (il blocco delle scritture e' per istanza).
pub const fn connectors_file() -> JsonFile<ConnectorsFile> {
    JsonFile::new(FILE_NAME, &STEPS).decoded_with(decode_connectors)
}

static CONNECTORS_FILE: JsonFile<ConnectorsFile> = connectors_file();

/// Legge `connectors.json` da `dir` (file assente = nessun connettore, nessun file creato).
pub fn load(dir: &Path) -> Loaded<ConnectorsFile> {
    CONNECTORS_FILE.load(dir)
}

/// Salva `connectors.json` (scrittura atomica; rifiutata se il file esistente non si e' potuto leggere).
pub fn save(dir: &Path, value: &ConnectorsFile) -> Result<(), String> {
    CONNECTORS_FILE.save(dir, value)
}

fn shown_id(entry: &Value) -> Option<String> {
    let id = entry.get("id")?.as_str()?;
    Some(id.chars().filter(|c| !c.is_control()).take(SHOWN_ID_MAX).collect())
}

/// Legge una sezione del file voce per voce: la voce non leggibile, non valida o
/// con un id gia' visto si esclude e si riporta in `dropped`.
fn decode_section<T: DeserializeOwned>(
    data: &mut Map<String, Value>,
    key: &str,
    noun: &str,
    id_of: fn(&T) -> &str,
    check: fn(&T) -> Result<(), String>,
    dropped: &mut Vec<String>,
) -> Result<Vec<T>, String> {
    let entries = match data.remove(key) {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Array(entries)) => entries,
        Some(_) => return Err(format!("«{key}» non è un elenco")),
    };
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(entries.len());
    for (index, entry) in entries.into_iter().enumerate() {
        let which = shown_id(&entry).map_or_else(|| format!("{noun} {}", index + 1), |id| format!("{noun} \"{id}\""));
        match serde_json::from_value::<T>(entry) {
            Ok(item) => match check(&item) {
                Ok(()) if seen.insert(id_of(&item).to_string()) => out.push(item),
                Ok(()) => dropped.push(format!("{which}: id duplicato")),
                Err(e) => dropped.push(format!("{which}: {e}")),
            },
            Err(e) => dropped.push(format!("{which}: {e}")),
        }
    }
    Ok(out)
}

/// Il decoder di `connectors.json`: voce per voce, con i riferimenti verificati.
fn decode_connectors(data: Value) -> Result<Decoded<ConnectorsFile>, String> {
    let mut map = match data {
        Value::Object(map) => map,
        Value::Null => Map::new(),
        _ => return Err("atteso un oggetto con istanze, regole e collegamenti".to_string()),
    };
    let mut dropped = Vec::new();
    let instances: Vec<ConnectorInstance> =
        decode_section(&mut map, "instances", "istanza", |i| &i.id, ConnectorInstance::check, &mut dropped)?;
    let mut routes: Vec<ConnectorRoute> =
        decode_section(&mut map, "routes", "regola", |r| &r.id, ConnectorRoute::check, &mut dropped)?;
    let mut links: Vec<ConnectorLink> =
        decode_section(&mut map, "links", "collegamento", |l| &l.id, ConnectorLink::check, &mut dropped)?;
    let known: HashSet<&str> = instances.iter().map(|i| i.id.as_str()).collect();
    routes.retain(|r| {
        let keep = known.contains(r.instance.as_str());
        if !keep {
            dropped.push(format!("regola \"{}\": istanza inesistente", r.id));
        }
        keep
    });
    links.retain(|l| {
        let keep = known.contains(l.instance.as_str());
        if !keep {
            dropped.push(format!("collegamento \"{}\": istanza inesistente", l.id));
        }
        keep
    });
    Ok(Decoded { value: ConnectorsFile { instances, routes, links }, dropped })
}

/// Il file di stato di un'istanza. Va tenuto vivo finche' l'istanza gira: il suo
/// `JsonFile` ricorda se il file su disco non si e' potuto leggere (e quindi non
/// va sovrascritto).
pub struct StateFile {
    file: JsonFile<InstanceState>,
    dir: PathBuf,
}

impl StateFile {
    /// Il file di stato di `instance` nella cartella dati dell'app. L'id deve essere un id di istanza.
    pub fn open(data_dir: &Path, instance: &str) -> Result<Self, String> {
        if !super::ids::valid_instance_id(instance) {
            return Err("id istanza non valido".to_string());
        }
        let file = JsonFile::named(format!("{instance}.json"), &STATE_STEPS);
        Ok(Self { file, dir: data_dir.join(STATE_DIR) })
    }

    /// Lo stato salvato. Assente = default (si parte da "adesso"); corrotto = backup,
    /// avviso e default.
    pub fn load(&self) -> Loaded<InstanceState> {
        self.file.load(&self.dir)
    }

    pub fn save(&self, state: &InstanceState) -> Result<(), String> {
        self.file.save(&self.dir, state)
    }

    /// Cancella il file di stato (l'istanza e' stata rimossa); un file assente non e' un errore.
    pub fn delete(&self) -> Result<(), String> {
        let path = self.dir.join(self.file.name());
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("impossibile eliminare lo stato del connettore: {e}")),
        }
    }
}

#[cfg(test)]
mod tests;
