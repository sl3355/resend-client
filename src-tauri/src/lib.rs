mod backup;
mod commands;
mod db;
mod model;
mod provider;
#[cfg(test)]
mod test_server;

use db::Store;
use model::*;
use provider::Provider;
use std::{path::PathBuf, sync::Mutex};
use tauri::Manager;

pub struct AppState {
    pub store: Mutex<Store>,
    pub provider: Provider,
    pub root: PathBuf,
    pub jobs: tokio::sync::Mutex<()>,
}
impl AppState {
    pub fn db(&self) -> Result<std::sync::MutexGuard<'_, Store>> {
        self.store
            .lock()
            .map_err(|_| "Local storage is temporarily unavailable. Restart the app.".into())
    }
    pub fn workspace_db(&self, id: &str) -> Result<std::sync::MutexGuard<'_, Store>> {
        let db = self.db()?;
        db.require_active(id)?;
        Ok(db)
    }
    pub fn secret(&self, profile: &str) -> Result<String> {
        keyring::Entry::new("com.resendclient.desktop", profile)
            .and_then(|e| e.get_password())
            .map_err(|_| {
                "Cannot read the credential from secure storage. Reconnect your account.".into()
            })
    }
    pub fn file_path(&self, profile: &str, id: &str) -> Result<PathBuf> {
        valid_id(profile)?;
        valid_id(id)?;
        Ok(self.root.join("attachments").join(profile).join(id))
    }
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let root = app.path().app_data_dir()?;
            std::fs::create_dir_all(&root)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))?;
            }
            let store = Store::open(&root.join("mail.sqlite")).map_err(std::io::Error::other)?;
            app.manage(AppState {
                store: Mutex::new(store),
                provider: Provider::new(),
                root,
                jobs: tokio::sync::Mutex::new(()),
            });
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut ticks = 0_u32;
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    let state = handle.state::<AppState>();
                    let connected = state
                        .db()
                        .and_then(|db| db.profile())
                        .map(|p| p.connected)
                        .unwrap_or(false);
                    if connected {
                        let _ = commands::process_outbox(&state).await;
                        if ticks.is_multiple_of(6) {
                            let _ = commands::sync(&state).await;
                        }
                    }
                    ticks = ticks.wrapping_add(1);
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::list_workspaces,
            commands::create_workspace,
            commands::switch_workspace,
            commands::rename_workspace,
            commands::confirm_action,
            commands::list_messages,
            commands::read_message,
            commands::connect_account,
            commands::disconnect_account,
            commands::save_settings,
            commands::refresh_connection,
            commands::save_draft,
            commands::delete_draft,
            commands::enqueue_send,
            commands::set_local_flags,
            commands::permanently_remove,
            commands::attach_files,
            commands::save_attachment,
            commands::cancel_intent,
            commands::retry_intent,
            commands::make_intent_copy,
            commands::open_external,
            backup::export_profile,
            backup::restore_profile
        ])
        .run(tauri::generate_context!())
        .expect("Failed to run Resend Client");
}
