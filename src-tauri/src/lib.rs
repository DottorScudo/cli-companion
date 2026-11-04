mod agents;
pub mod automation;
mod awake;
pub mod board;
mod bots;
pub mod ipc;
mod launch_settings;
mod logging;
pub mod mcp;
mod net;
mod platform;
mod pty;
pub mod schedule;
mod session;
mod startup;
pub mod store;
mod toast;
mod ttl_cache;
pub mod updater;
pub mod user_skills;
mod validate;
mod workspaces;
mod wsl;

use automation::AutomationState;
use pty::PtyState;
use workspaces::WorkspaceStore;

/// Esegue lavoro bloccante (processi esterni, rete, filesystem) fuori dal
/// main thread. I comandi Tauri sincroni girano sul main thread: un comando
/// lento li' congela l'intera finestra (era il blocco all'avvio causato dal
/// controllo aggiornamenti via curl).
pub(crate) async fn blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| format!("operazione interrotta: {e}"))?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    startup::begin();
    tauri::Builder::default()
        .plugin(logging::plugin())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(WorkspaceStore::default())
        .manage(PtyState::default())
        .manage(launch_settings::new_store())
        .manage(bots::new_store())
        .manage(AutomationState::default())
        .setup(|app| {
            startup::mark("setup started");
            store::install_emitter(app.handle());
            startup::timed("legacy data migrated", || store::legacy::migrate_legacy_data(app));
            startup::timed("integrations loaded", || agents::load_integrations(app));
            startup::timed("workspaces loaded", || workspaces::load_from_disk(app));
            startup::timed("launch settings loaded", || launch_settings::load_from_disk(app));
            startup::timed("bots loaded", || bots::load_from_disk(app));
            startup::timed("boards loaded", || automation::load_boards(app));
            startup::timed("automation started", || automation::auto::init(app));
            automation::skills::restore_in_background(automation::TauriHost(app.handle().clone()));
            startup::timed("ipc listener started", || {
                ipc::start_listener(app.handle(), std::sync::Arc::new(automation::IpcBridge(app.handle().clone())))
            });
            Ok(())
        })
        .on_page_load(|_, payload| startup::page_load(payload.event(), payload.url().as_str()))
        .invoke_handler(tauri::generate_handler![
            workspaces::list_workspaces,
            workspaces::add_workspace,
            workspaces::rename_workspace,
            workspaces::set_worker_permission,
            workspaces::set_workspace_agent,
            workspaces::set_worker_model,
            workspaces::set_worker_permission_rules,
            workspaces::set_worker_rules,
            workspaces::set_repo_config,
            workspaces::set_delivery,
            workspaces::set_use_schedule,
            workspaces::reorder_workspaces,
            workspaces::set_workspace_archived,
            workspaces::remove_workspace,
            workspaces::add_launch,
            workspaces::update_launch,
            workspaces::remove_launch,
            workspaces::set_default_launch,
            launch_settings::get_launch_settings,
            launch_settings::set_global_default_launch,
            launch_settings::set_show_usage,
            bots::get_bot_settings,
            bots::set_bot_settings,
            bots::reset_bot_names,
            bots::set_bot_image,
            bots::remove_bot_image,
            agents::builtin_launches,
            workspaces::open_workspace_folder,
            wsl::list_wsl_distros,
            wsl::check_wsl_path,
            pty::pty_spawn,
            pty::pty_write,
            pty::pty_resize,
            pty::pty_close,
            ipc::test_agent_notify,
            ipc::ipc_status,
            automation::automation_tab_closed,
            automation::automation_sessions,
            automation::board_get,
            automation::board_set,
            automation::task_create,
            automation::task_update,
            automation::task_action,
            automation::task_move,
            automation::proposal_decide,
            automation::settings_proposal_decide,
            automation::workspace_detect_repos,
            automation::e2e_run_repo,
            automation::e2e_last_result,
            automation::e2e_open_report,
            automation::auto::schedule_get,
            automation::auto::schedule_set,
            automation::auto::frontend_ready,
            toast::show_agent_notification,
            agents::agent_env_status,
            agents::agent_profiles,
            agents::agent_usage_limits,
            agents::agent_session_usage,
            agents::install_agent_plugin,
            agents::integrations_list,
            agents::set_integration_enabled,
            agents::set_default_integration,
            agents::agent_plugins_outdated,
            agents::agent_plugins_update,
            agents::remove_agent_plugin,
            store::warnings::store_warnings,
            session::load_session,
            session::save_session,
            session::save_scrollback,
            session::load_scrollback,
            session::delete_scrollback,
            session::prune_scrollback,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                // Salvataggi di board ancora in coda (coalescenza).
                automation::flush_on_exit(app);
                // Nessuna shell resta viva dopo la chiusura della finestra.
                pty::close_all_on_exit(app);
            }
        });
}
