mod commands;
mod events;
mod state;

use tauri::{Builder, Manager, Runtime};

/// Registers every command. Shared by the app and the IPC tests.
fn with_commands<R: Runtime>(builder: Builder<R>) -> Builder<R> {
    builder.invoke_handler(tauri::generate_handler![
        commands::app_info,
        commands::local_user,
        commands::rename_local_user,
        commands::list_discussions,
        commands::get_discussion,
        commands::start_discussion,
        commands::add_post,
        commands::rename_discussion,
        commands::set_archived,
        commands::save_draft,
    ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    with_commands(tauri::Builder::default())
        .setup(|app| {
            app.manage(state::AppState::open(app.handle()));
            events::start_clock(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Nimata");
}

#[cfg(test)]
mod ipc_tests;
