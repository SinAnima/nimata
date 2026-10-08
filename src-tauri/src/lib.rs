mod commands;
mod events;
mod generation;
mod secrets;
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
        commands::edit_post,
        commands::post_revisions,
        commands::delete_post,
        commands::delete_discussion,
        commands::database_status,
        commands::export_discussion,
        commands::backup_database,
        commands::restore_database,
        commands::providers,
        commands::save_api_key,
        commands::remove_api_key,
        commands::provider_models,
        commands::set_model,
        commands::ask_model,
        commands::cancel_reply,
        commands::retry_reply,
        commands::reply_details,
        commands::set_model_aliases,
        commands::default_model,
        commands::set_default_model,
        commands::add_endpoint,
        commands::update_endpoint,
        commands::remove_endpoint,
        commands::preview_context,
        commands::search,
        commands::record_search,
        commands::recent_searches,
        commands::clear_recent_searches,
        commands::export_markdown,
        commands::export_archive,
        commands::import_file,
    ])
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Before anything creates an HTTP client, including Tauri itself.
    nimata_core::providers::install_crypto_provider();
    with_commands(tauri::Builder::default())
        .plugin(tauri_plugin_dialog::init())
        .manage(secrets::Keys::os())
        .manage(generation::Generations::default())
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
