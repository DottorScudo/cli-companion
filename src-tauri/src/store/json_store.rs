//! Stato in memoria di un file JSON, gestito da Tauri: letture rapide dal main
//! thread, modifiche serializzate e salvate prima di essere pubblicate.

use super::file::JsonFile;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::path::Path;
use std::sync::Mutex;

pub struct JsonStore<T: 'static> {
    file: &'static JsonFile<T>,
    data: Mutex<T>,
    /// Serializza le modifiche: l'I/O avviene fuori dal lock `data` (i lettori girano sul main thread).
    save: Mutex<()>,
}

impl<T: Serialize + DeserializeOwned + Default + Clone> JsonStore<T> {
    pub fn new(file: &'static JsonFile<T>) -> Self {
        Self { file, data: Mutex::new(T::default()), save: Mutex::new(()) }
    }

    /// Copia del dato corrente.
    pub fn get(&self) -> T {
        self.data.lock().unwrap().clone()
    }

    /// Legge il dato corrente senza clonarlo: `f` gira sotto il lock, quindi deve
    /// essere breve e non toccare lo store.
    pub fn read<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(&self.data.lock().unwrap())
    }

    /// Carica dal disco (avvisi e quarantena sono gestiti da `JsonFile::load`).
    pub fn load(&self, dir: &Path) {
        let loaded = self.file.load(dir).value;
        *self.data.lock().unwrap() = loaded;
    }

    /// Applica `f` a una copia, la salva e solo poi la pubblica in memoria: se
    /// `f` o il salvataggio falliscono lo stato resta invariato e nulla viene scritto.
    pub fn update(&self, dir: &Path, f: impl FnOnce(&mut T) -> Result<(), String>) -> Result<T, String> {
        let _save = self.save.lock().unwrap();
        let mut next = self.get();
        f(&mut next)?;
        self.file.save(dir, &next)?;
        *self.data.lock().unwrap() = next.clone();
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::adopt_envelope;
    use serde::Deserialize;
    use std::path::PathBuf;

    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    struct Counter {
        n: u32,
    }

    static COUNTER_FILE: JsonFile<Counter> = JsonFile::new("counter.json", &[adopt_envelope]);
    static BLOCKED_FILE: JsonFile<Counter> = JsonFile::new("blocked.json", &[adopt_envelope]);

    fn temp_dir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("cc-jstore-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn update_persists_and_returns_the_new_value() {
        let d = temp_dir("persist");
        let store = JsonStore::new(&COUNTER_FILE);
        assert_eq!(store.get(), Counter::default());
        let next = store
            .update(&d, |c| {
                c.n = 5;
                Ok(())
            })
            .unwrap();
        assert_eq!(next, Counter { n: 5 });
        assert_eq!(store.get(), Counter { n: 5 });
        let reloaded = JsonStore::new(&COUNTER_FILE);
        reloaded.load(&d);
        assert_eq!(reloaded.get(), Counter { n: 5 });
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn read_borrows_the_current_value() {
        let d = temp_dir("read");
        let store = JsonStore::new(&COUNTER_FILE);
        assert_eq!(store.read(|c| c.n), 0);
        store
            .update(&d, |c| {
                c.n = 3;
                Ok(())
            })
            .unwrap();
        assert_eq!(store.read(|c| c.n), 3);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn failing_closure_changes_nothing() {
        let d = temp_dir("closure");
        let store = JsonStore::new(&COUNTER_FILE);
        store
            .update(&d, |c| {
                c.n = 1;
                Ok(())
            })
            .unwrap();
        let before = std::fs::read_to_string(d.join("counter.json")).unwrap();
        let err = store
            .update(&d, |c| {
                c.n = 99;
                Err("rifiutato".to_string())
            })
            .unwrap_err();
        assert_eq!(err, "rifiutato");
        assert_eq!(store.get(), Counter { n: 1 });
        assert_eq!(std::fs::read_to_string(d.join("counter.json")).unwrap(), before);
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn failing_save_leaves_memory_unchanged() {
        let d = temp_dir("blocked");
        // File illeggibile (cartella): il caricamento blocca le scritture.
        std::fs::create_dir_all(d.join("blocked.json")).unwrap();
        let store = JsonStore::new(&BLOCKED_FILE);
        store.load(&d);
        assert_eq!(store.get(), Counter::default());
        let r = store.update(&d, |c| {
            c.n = 7;
            Ok(())
        });
        assert!(r.is_err());
        assert_eq!(store.get(), Counter::default());
        assert!(d.join("blocked.json").is_dir());
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn concurrent_updates_do_not_lose_increments() {
        static SHARED_FILE: JsonFile<Counter> = JsonFile::new("shared.json", &[adopt_envelope]);
        const THREADS: u32 = 4;
        const PER_THREAD: u32 = 10;
        let d = temp_dir("threads");
        let store = JsonStore::new(&SHARED_FILE);
        std::thread::scope(|s| {
            for _ in 0..THREADS {
                s.spawn(|| {
                    for _ in 0..PER_THREAD {
                        store
                            .update(&d, |c| {
                                c.n += 1;
                                Ok(())
                            })
                            .unwrap();
                    }
                });
            }
        });
        assert_eq!(store.get().n, THREADS * PER_THREAD);
        let reloaded = JsonStore::new(&SHARED_FILE);
        reloaded.load(&d);
        assert_eq!(reloaded.get().n, THREADS * PER_THREAD);
        let _ = std::fs::remove_dir_all(&d);
    }
}
