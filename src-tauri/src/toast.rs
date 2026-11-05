//! Toast Windows cliccabili per gli eventi degli agenti e per i thread
//! dell'Agents HUB. Il plugin notification non espone il click su desktop: qui
//! si usa direttamente `tauri-winrt-notification` (lo stesso crate sotto
//! notify-rust) con `on_activated`, che porta in primo piano la finestra e
//! avvisa il frontend. Le due varianti condividono AUMID, costruzione della
//! toast e attivazione (`show_clickable`) e differiscono per l'evento emesso.

use crate::hub::validate_thread_ref;
use crate::ipc::is_valid_tab_id;

/// Evento Tauri emesso al click su una toast di un agente: `{ tab_id }`.
#[cfg(windows)]
const ACTIVATED_EVENT: &str = "notification-activated";

/// Evento Tauri emesso al click su una toast dell'HUB: `{ channel, thread_id }`.
#[cfg(windows)]
const HUB_ACTIVATED_EVENT: &str = "hub-notification-activated";

/// AUMID della toast. Da installata e' l'identifier dell'app (registrato dal
/// bundler NSIS/MSI); da `target\debug|release` nessuna app e' registrata e si
/// ripiega su quello di PowerShell. Stessa logica di tauri-plugin-notification.
#[cfg(windows)]
fn app_id(identifier: &str) -> String {
    use tauri_winrt_notification::Toast;
    let portable = tauri::utils::platform::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.display().to_string()))
        .is_some_and(|dir| {
            let sep = std::path::MAIN_SEPARATOR;
            dir.ends_with(&format!("{sep}target{sep}debug")) || dir.ends_with(&format!("{sep}target{sep}release"))
        });
    if portable {
        Toast::POWERSHELL_APP_ID.to_string()
    } else {
        identifier.to_string()
    }
}

/// Porta la finestra principale in primo piano.
#[cfg(windows)]
fn bring_to_front(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Divide il corpo alla prima riga: la prima va nel primo testo della toast, il
/// resto (se c'e') nel secondo.
#[cfg(windows)]
fn split_body(body: &str) -> (&str, Option<&str>) {
    match body.split_once('\n') {
        Some((first, rest)) => (first, Some(rest)),
        None => (body, None),
    }
}

/// Mostra una toast cliccabile; al click porta in primo piano la finestra e
/// poi chiama `on_click`. `second` e' il secondo rigo, se serve.
/// Solo Windows: altrove errore, e il frontend ripiega sul plugin.
#[cfg(windows)]
async fn show_clickable(
    app: tauri::AppHandle,
    title: String,
    first: String,
    second: Option<String>,
    on_click: impl Fn(&tauri::AppHandle) + Send + 'static,
) -> Result<(), String> {
    // WinRT fuori dal main thread (async + thread bloccante).
    crate::blocking(move || {
        use tauri_winrt_notification::Toast;
        let id = app_id(&app.config().identifier);
        let handle = app.clone();
        let mut toast = Toast::new(&id).title(&title).text1(&first);
        if let Some(second) = &second {
            toast = toast.text2(second);
        }
        toast
            // Il suono lo riproduce gia' l'app.
            .sound(None)
            .on_activated(move |_| {
                bring_to_front(&handle);
                on_click(&handle);
                Ok(())
            })
            .show()
            .map_err(|e| format!("toast non mostrata: {e}"))
    })
    .await
}

#[cfg(not(windows))]
async fn show_clickable(
    _app: tauri::AppHandle,
    _title: String,
    _first: String,
    _second: Option<String>,
    _on_click: impl Fn(&tauri::AppHandle) + Send + 'static,
) -> Result<(), String> {
    Err("toast cliccabile solo su Windows".to_string())
}

/// Mostra la toast per il tab `tab_id`; al click focus + `notification-activated`.
/// Solo Windows: altrove errore, e il frontend ripiega sul plugin.
#[tauri::command]
pub async fn show_agent_notification(
    app: tauri::AppHandle,
    tab_id: String,
    title: String,
    body: String,
) -> Result<(), String> {
    if !is_valid_tab_id(&tab_id) {
        return Err("id tab non valido".to_string());
    }
    show_clickable(app, title, body, None, move |_handle| {
        #[cfg(windows)]
        {
            use tauri::Emitter;
            let _ = _handle.emit(ACTIVATED_EVENT, crate::automation::TabEvent { tab_id: tab_id.clone() });
        }
    })
    .await
}

/// Mostra la toast per un thread dell'HUB; al click focus +
/// `hub-notification-activated` con `{ channel, thread_id }`. Il primo rigo di
/// `body` e' il primo testo, il secondo il secondo. Solo Windows: altrove
/// errore, e il frontend ripiega sul plugin (senza click).
#[tauri::command]
pub async fn show_hub_notification(
    app: tauri::AppHandle,
    channel: String,
    thread_id: String,
    title: String,
    body: String,
) -> Result<(), String> {
    let thread = validate_thread_ref(&channel, &thread_id)?;
    #[cfg(windows)]
    let (first, second) = {
        let (first, second) = split_body(&body);
        (first.to_string(), second.map(str::to_string))
    };
    #[cfg(not(windows))]
    let (first, second) = (body, None);
    show_clickable(app, title, first, second, move |_handle| {
        #[cfg(windows)]
        {
            use tauri::Emitter;
            let _ = _handle.emit(HUB_ACTIVATED_EVENT, thread.clone());
        }
        #[cfg(not(windows))]
        let _ = &thread;
    })
    .await
}

#[cfg(all(test, windows))]
mod tests {
    use super::split_body;

    #[test]
    fn the_first_line_goes_first_and_the_rest_second() {
        assert_eq!(split_body("Generale · Piano\nFatto"), ("Generale · Piano", Some("Fatto")));
        assert_eq!(split_body("Solo una riga"), ("Solo una riga", None));
        assert_eq!(split_body("a\nb\nc"), ("a", Some("b\nc")));
    }
}
