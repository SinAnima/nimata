mod commands;
mod events;

use nimata_core::fixtures;
use nimata_core::time::UnixMillis;

pub struct AppState {
    pub fixtures: fixtures::Fixtures,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            fixtures: fixtures::build(UnixMillis::now()),
        })
        .setup(|app| {
            events::start_clock(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::list_discussions,
            commands::get_discussion,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Nimata");
}
