//! Tempi dell'avvio nel log: ogni passo (setup, caricamento della pagina,
//! `frontend_ready`) scrive i millisecondi trascorsi dall'inizio di `run`.
//! Servono a capire dove va il tempo tra l'apertura della finestra e la UI.

use std::sync::OnceLock;
use std::time::Instant;
use tauri::webview::PageLoadEvent;

/// Istante d'inizio di `run`.
static STARTED: OnceLock<Instant> = OnceLock::new();

/// Fissa l'inizio dell'avvio: la prima chiamata vince.
pub fn begin() {
    STARTED.get_or_init(Instant::now);
}

/// Millisecondi da `begin` (0 se `begin` non e' stato chiamato).
fn elapsed_ms() -> u128 {
    STARTED.get().map_or(0, |t| t.elapsed().as_millis())
}

/// Scrive nel log che `step` e' concluso, con il tempo dall'inizio.
pub fn mark(step: &str) {
    log::info!("startup: {step} at +{} ms", elapsed_ms());
}

/// Esegue un passo dell'avvio e ne scrive la fine nel log.
pub fn timed<T>(step: &str, f: impl FnOnce() -> T) -> T {
    let out = f();
    mark(step);
    out
}

/// Inizio e fine di ogni caricamento della pagina (anche le ricariche).
pub fn page_load(event: PageLoadEvent, url: &str) {
    let phase = match event {
        PageLoadEvent::Started => "started",
        PageLoadEvent::Finished => "finished",
    };
    mark(&format!("page load {phase} ({url})"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timed_returns_the_value_of_the_step() {
        begin();
        assert_eq!(timed("test step", || 42), 42);
    }

    #[test]
    fn elapsed_counts_from_the_first_begin() {
        begin();
        std::thread::sleep(std::time::Duration::from_millis(5));
        begin();
        assert!(elapsed_ms() >= 5, "una seconda chiamata non azzera l'inizio");
    }
}
