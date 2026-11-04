//! Un file JSON versionato: caricamento con migrazioni e quarantena, salvataggio atomico.

use super::atomic::write_atomic;
use super::backup;
use super::envelope;
use super::migrate::{run_chain, Migration};
use super::warnings::{self, StoreWarning, WarningKind};
use chrono::Utc;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use std::io::ErrorKind;
use std::marker::PhantomData;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

/// Descrizione di un file di dati (nome + catena di migrazioni). Pensato per
/// vivere in una `static`: il flag di blocco scritture e' per istanza.
pub struct JsonFile<T> {
    name: &'static str,
    steps: &'static [Migration],
    check: fn(&T) -> Result<(), String>,
    /// Alzato quando il file esistente non e' stato ne' letto ne' messo da
    /// parte: salvare lo distruggerebbe.
    write_blocked: AtomicBool,
    _t: PhantomData<fn() -> T>,
}

/// Esito di `load`: il valore da usare e l'eventuale avviso.
pub struct Loaded<T> {
    pub value: T,
    pub warning: Option<StoreWarning>,
}

/// Perche' il contenuto di un file non e' utilizzabile.
enum Rejection {
    Corrupt(String),
    /// Versione dello schema piu' recente di quella supportata.
    Newer(u32),
}

fn accept_all<T>(_: &T) -> Result<(), String> {
    Ok(())
}

impl<T: Serialize + DeserializeOwned + Default> JsonFile<T> {
    /// `steps` non puo' essere vuoto: il primo passo e' 0 -> 1 (la busta).
    pub const fn new(name: &'static str, steps: &'static [Migration]) -> Self {
        assert!(!steps.is_empty(), "almeno un passo di migrazione (0 -> 1)");
        Self { name, steps, check: accept_all::<T>, write_blocked: AtomicBool::new(false), _t: PhantomData }
    }

    /// Validazione semantica dopo la deserializzazione: se fallisce il file e'
    /// trattato come corrotto.
    pub const fn validated(mut self, check: fn(&T) -> Result<(), String>) -> Self {
        self.check = check;
        self
    }

    pub fn name(&self) -> &'static str {
        self.name
    }

    /// Versione corrente dello schema = numero dei passi di migrazione.
    pub fn schema_version(&self) -> u32 {
        self.steps.len() as u32
    }

    /// Legge `dir/<name>`. File assente = default senza avvisi; file inutilizzabile =
    /// default con avviso, e il file originale non viene mai distrutto.
    pub fn load(&self, dir: &Path) -> Loaded<T> {
        let bytes = match std::fs::read(dir.join(self.name)) {
            Ok(b) => b,
            Err(e) if e.kind() == ErrorKind::NotFound => return Loaded { value: T::default(), warning: None },
            Err(e) => return self.unreadable(&e),
        };
        match self.decode(&bytes) {
            Ok(value) => Loaded { value, warning: None },
            Err(Rejection::Corrupt(reason)) => self.set_aside(dir, WarningKind::Corrupt, &reason),
            Err(Rejection::Newer(found)) => {
                let reason = format!("scritto da una versione più recente dell'app (schema {found})");
                self.set_aside(dir, WarningKind::NewerVersion, &reason)
            }
        }
    }

    /// Salva `value` nella busta della versione corrente (scrittura atomica).
    pub fn save(&self, dir: &Path, value: &T) -> Result<(), String> {
        if self.write_blocked.load(Ordering::SeqCst) {
            return Err(format!(
                "{} non è stato letto correttamente: il salvataggio è disabilitato per non sovrascrivere i dati \
                 su disco. Sistema il file e riavvia l'app.",
                self.name
            ));
        }
        let fail = |e: &dyn std::fmt::Display| format!("impossibile salvare {}: {e}", self.name);
        std::fs::create_dir_all(dir).map_err(|e| fail(&e))?;
        let raw = envelope::to_pretty(self.schema_version(), value).map_err(|e| fail(&e))?;
        write_atomic(&dir.join(self.name), &raw).map_err(|e| fail(&e))
    }

    fn decode(&self, bytes: &[u8]) -> Result<T, Rejection> {
        let corrupt = |reason: String| Rejection::Corrupt(reason);
        let text = std::str::from_utf8(bytes).map_err(|e| corrupt(format!("UTF-8 non valido: {e}")))?;
        let raw: Value = serde_json::from_str(text).map_err(|e| corrupt(format!("JSON non valido: {e}")))?;
        let (version, data) = envelope::split(raw);
        if version > self.schema_version() {
            return Err(Rejection::Newer(version));
        }
        let data = run_chain(data, version, self.steps).map_err(corrupt)?;
        let value: T = serde_json::from_value(data).map_err(|e| corrupt(format!("dati non validi: {e}")))?;
        (self.check)(&value).map_err(corrupt)?;
        Ok(value)
    }

    /// Mette da parte il file inutilizzabile come backup datato. Se nemmeno
    /// il rename riesce, il file resta dov'e' e i salvataggi vengono bloccati.
    fn set_aside(&self, dir: &Path, kind: WarningKind, reason: &str) -> Loaded<T> {
        match backup::quarantine(dir, self.name, Utc::now()) {
            Ok(saved) => {
                let message = format!(
                    "Il file {} non è utilizzabile ({reason}): è stato salvato come {saved} e riparte dai valori \
                     predefiniti.",
                    self.name
                );
                log::warn!("{message}");
                self.finish(kind, message, Some(saved))
            }
            Err(e) => {
                self.write_blocked.store(true, Ordering::SeqCst);
                let message = format!(
                    "Il file {} non è utilizzabile ({reason}) e non è stato possibile metterlo da parte ({e}): \
                     resta dov'è e il suo salvataggio è disabilitato.",
                    self.name
                );
                log::error!("{message}");
                self.finish(kind, message, None)
            }
        }
    }

    fn unreadable(&self, e: &std::io::Error) -> Loaded<T> {
        self.write_blocked.store(true, Ordering::SeqCst);
        let message = format!(
            "Impossibile leggere {} ({e}): il file non è stato toccato e il suo salvataggio è disabilitato. \
             Sistema il file e riavvia l'app.",
            self.name
        );
        log::error!("{message}");
        self.finish(WarningKind::Unreadable, message, None)
    }

    fn finish(&self, kind: WarningKind, message: String, backup: Option<String>) -> Loaded<T> {
        let warning =
            StoreWarning { file: self.name.to_string(), kind, message, backup, at: Utc::now().timestamp_millis() };
        warnings::report(warning.clone());
        Loaded { value: T::default(), warning: Some(warning) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;
    use serde_json::json;
    use std::path::PathBuf;
    use std::time::Duration;

    /// Piccolo dato di prova: `renamed` si chiamava `old` nella v0, `level` e' stato aggiunto nella v2.
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct Doc {
        #[serde(default)]
        renamed: String,
        #[serde(default = "default_level")]
        level: u32,
    }

    fn default_level() -> u32 {
        3
    }

    impl Default for Doc {
        fn default() -> Self {
            Doc { renamed: String::new(), level: default_level() }
        }
    }

    fn rename_old(mut v: Value) -> Result<Value, String> {
        let obj = v.as_object_mut().ok_or("atteso un oggetto")?;
        if let Some(old) = obj.remove("old") {
            obj.insert("renamed".into(), old);
        }
        Ok(v)
    }

    fn add_level(mut v: Value) -> Result<Value, String> {
        let obj = v.as_object_mut().ok_or("atteso un oggetto")?;
        // Dipende dal passo precedente: senza il campo gia' rinominato il livello e' 1.
        let level = if obj.contains_key("renamed") { 10 } else { 1 };
        obj.entry("level").or_insert(json!(level));
        Ok(v)
    }

    fn always_fails(_: Value) -> Result<Value, String> {
        Err("rotto".to_string())
    }

    fn file() -> JsonFile<Doc> {
        JsonFile::new("doc.json", &[rename_old, add_level])
    }

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cc-file-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> =
            std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
        v.sort();
        v
    }

    fn backups(dir: &Path) -> Vec<String> {
        names(dir).into_iter().filter(|n| n.starts_with("doc.json.bak-")).collect()
    }

    #[test]
    fn missing_file_gives_default_without_creating_anything() {
        let d = temp_dir("missing");
        let loaded = file().load(&d);
        assert_eq!(loaded.value, Doc::default());
        assert!(loaded.warning.is_none());
        assert!(names(&d).is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn schema_version_is_the_number_of_steps() {
        assert_eq!(file().schema_version(), 2);
        assert_eq!(file().name(), "doc.json");
    }

    #[test]
    fn save_writes_the_envelope_and_roundtrips() {
        let d = temp_dir("roundtrip");
        let f = file();
        let doc = Doc { renamed: "x".into(), level: 9 };
        f.save(&d, &doc).unwrap();
        let raw: Value = serde_json::from_str(&std::fs::read_to_string(d.join("doc.json")).unwrap()).unwrap();
        assert_eq!(raw, json!({"schema_version": 2, "data": {"renamed": "x", "level": 9}}));
        let loaded = f.load(&d);
        assert_eq!(loaded.value, doc);
        assert!(loaded.warning.is_none());
        assert_eq!(names(&d), vec!["doc.json".to_string()]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn save_creates_the_data_dir() {
        let d = temp_dir("mkdir").join("nuova").join("annidata");
        file().save(&d, &Doc::default()).unwrap();
        assert!(d.join("doc.json").is_file());
        let _ = std::fs::remove_dir_all(d.parent().unwrap().parent().unwrap());
    }

    #[test]
    fn legacy_bare_file_runs_the_whole_chain_in_order() {
        let d = temp_dir("legacy");
        std::fs::write(d.join("doc.json"), r#"{"old":"vecchio"}"#).unwrap();
        let loaded = file().load(&d);
        // Rinomina (0->1) e poi aggiunta del livello (1->2): se l'ordine fosse
        // invertito il livello varrebbe 1.
        assert_eq!(loaded.value, Doc { renamed: "vecchio".into(), level: 10 });
        assert!(loaded.warning.is_none());
        assert!(backups(&d).is_empty());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn older_envelope_only_runs_the_missing_steps() {
        let d = temp_dir("older");
        std::fs::write(d.join("doc.json"), r#"{"schema_version":1,"data":{"renamed":"gia","old":"ignorato"}}"#)
            .unwrap();
        let loaded = file().load(&d);
        // Il passo di rinomina (0->1) non gira: `old` non viene toccato.
        assert_eq!(loaded.value, Doc { renamed: "gia".into(), level: 10 });
        assert!(loaded.warning.is_none());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn newer_version_is_set_aside_and_never_destroyed() {
        let d = temp_dir("newer");
        let f = file();
        let original = r#"{"schema_version":99,"data":{"renamed":"futuro"}}"#;
        std::fs::write(d.join("doc.json"), original).unwrap();
        let loaded = f.load(&d);
        assert_eq!(loaded.value, Doc::default());
        let w = loaded.warning.unwrap();
        assert_eq!(w.kind, WarningKind::NewerVersion);
        assert_eq!(w.file, "doc.json");
        let backup = w.backup.unwrap();
        assert!(w.message.contains(&backup));
        assert!(!d.join("doc.json").exists());
        assert_eq!(std::fs::read_to_string(d.join(&backup)).unwrap(), original);
        // Un salvataggio successivo funziona e non tocca il backup.
        f.save(&d, &Doc { renamed: "nuovo".into(), level: 1 }).unwrap();
        assert_eq!(std::fs::read_to_string(d.join(&backup)).unwrap(), original);
        assert_eq!(f.load(&d).value.renamed, "nuovo");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn corrupt_json_gets_a_dated_backup_then_a_fresh_file() {
        let d = temp_dir("corrupt");
        let f = file();
        std::fs::write(d.join("doc.json"), "{ non json").unwrap();
        let loaded = f.load(&d);
        assert_eq!(loaded.value, Doc::default());
        let w = loaded.warning.unwrap();
        assert_eq!(w.kind, WarningKind::Corrupt);
        let backup = w.backup.unwrap();
        assert!(backup.starts_with("doc.json.bak-"), "{backup}");
        assert_eq!(std::fs::read_to_string(d.join(&backup)).unwrap(), "{ non json");
        assert!(!d.join("doc.json").exists());
        // Nessuna riscrittura implicita: il file nuovo nasce solo da un save.
        assert_eq!(backups(&d).len(), 1);
        f.save(&d, &Doc::default()).unwrap();
        assert!(d.join("doc.json").is_file());
        assert!(w.at > 0);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn two_consecutive_corruptions_keep_both_backups() {
        let d = temp_dir("twice");
        let f = file();
        std::fs::write(d.join("doc.json"), "prima").unwrap();
        let first = f.load(&d).warning.unwrap().backup.unwrap();
        std::thread::sleep(Duration::from_millis(2));
        std::fs::write(d.join("doc.json"), "seconda").unwrap();
        let second = f.load(&d).warning.unwrap().backup.unwrap();
        assert_ne!(first, second);
        assert_eq!(std::fs::read_to_string(d.join(first)).unwrap(), "prima");
        assert_eq!(std::fs::read_to_string(d.join(second)).unwrap(), "seconda");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn only_the_newest_backups_are_kept() {
        let d = temp_dir("many");
        let f = file();
        for i in 0..7 {
            std::fs::write(d.join("doc.json"), format!("rotto {i}")).unwrap();
            assert!(f.load(&d).warning.unwrap().backup.is_some());
            std::thread::sleep(Duration::from_millis(2));
        }
        let kept = backups(&d);
        assert_eq!(kept.len(), 5);
        let contents: Vec<String> = kept.iter().map(|n| std::fs::read_to_string(d.join(n)).unwrap()).collect();
        assert_eq!(contents, (2..7).map(|i| format!("rotto {i}")).collect::<Vec<_>>());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn invalid_utf8_is_corrupt() {
        let d = temp_dir("utf8");
        let bytes: &[u8] = &[b'{', 0xff, 0xfe, b'}'];
        std::fs::write(d.join("doc.json"), bytes).unwrap();
        let w = file().load(&d).warning.unwrap();
        assert_eq!(w.kind, WarningKind::Corrupt);
        assert_eq!(std::fs::read(d.join(w.backup.unwrap())).unwrap(), bytes);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn wrong_shape_is_corrupt() {
        let d = temp_dir("shape");
        std::fs::write(d.join("doc.json"), r#"{"schema_version":2,"data":{"level":"alto"}}"#).unwrap();
        let w = file().load(&d).warning.unwrap();
        assert_eq!(w.kind, WarningKind::Corrupt);
        assert!(w.backup.is_some());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn failing_migration_step_is_corrupt() {
        let d = temp_dir("badstep");
        let f: JsonFile<Doc> = JsonFile::new("doc.json", &[rename_old, always_fails]);
        std::fs::write(d.join("doc.json"), r#"{"old":"x"}"#).unwrap();
        let loaded = f.load(&d);
        assert_eq!(loaded.value, Doc::default());
        let w = loaded.warning.unwrap();
        assert_eq!(w.kind, WarningKind::Corrupt);
        assert!(w.message.contains("1 -> 2"), "{}", w.message);
        assert_eq!(std::fs::read_to_string(d.join(w.backup.unwrap())).unwrap(), r#"{"old":"x"}"#);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn validation_failure_is_corrupt() {
        fn level_below_five(d: &Doc) -> Result<(), String> {
            if d.level < 5 {
                Ok(())
            } else {
                Err("livello troppo alto".to_string())
            }
        }
        let d = temp_dir("validated");
        let f = file().validated(level_below_five);
        f.save(&d, &Doc { renamed: String::new(), level: 4 }).unwrap();
        assert_eq!(f.load(&d).value.level, 4);
        std::fs::write(d.join("doc.json"), r#"{"schema_version":2,"data":{"level":8}}"#).unwrap();
        let w = f.load(&d).warning.unwrap();
        assert_eq!(w.kind, WarningKind::Corrupt);
        assert!(w.message.contains("livello troppo alto"));
        assert!(!d.join("doc.json").exists());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn unreadable_file_is_untouched_and_saving_is_refused() {
        let d = temp_dir("unreadable");
        let f = file();
        // Una cartella al posto del file: la lettura fallisce (non NotFound) ovunque.
        std::fs::create_dir_all(d.join("doc.json")).unwrap();
        let loaded = f.load(&d);
        assert_eq!(loaded.value, Doc::default());
        let w = loaded.warning.unwrap();
        assert_eq!(w.kind, WarningKind::Unreadable);
        assert!(w.backup.is_none());
        assert!(backups(&d).is_empty());
        assert!(d.join("doc.json").is_dir());
        assert!(f.save(&d, &Doc::default()).is_err());
        assert!(d.join("doc.json").is_dir());
        assert_eq!(names(&d), vec!["doc.json".to_string()]);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn save_reports_directory_creation_failures() {
        let d = temp_dir("badparent");
        let blocker = d.join("file");
        std::fs::write(&blocker, "x").unwrap();
        let err = file().save(&blocker.join("sotto"), &Doc::default()).unwrap_err();
        assert!(err.contains("doc.json"), "{err}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    #[should_panic(expected = "almeno un passo")]
    fn empty_step_list_is_rejected() {
        let steps: &'static [Migration] = &[];
        let _ = JsonFile::<Doc>::new("doc.json", steps);
    }
}
