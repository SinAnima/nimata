use nimata_core::{DiscussionSummary, DiscussionView};
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

use crate::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    tauri_version: &'static str,
    data_dir: Option<String>,
    platform: &'static str,
}

#[tauri::command]
pub fn app_info(app: AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        tauri_version: tauri::VERSION,
        data_dir: app
            .path()
            .app_data_dir()
            .ok()
            .map(|p| p.display().to_string()),
        platform: std::env::consts::OS,
    }
}

/// Discussions ordered by latest activity, newest first.
#[tauri::command]
pub fn list_discussions(state: State<'_, AppState>) -> Vec<DiscussionSummary> {
    let mut list: Vec<_> = state
        .fixtures
        .views
        .iter()
        .map(DiscussionView::summary)
        .collect();
    list.sort_by_key(|d| std::cmp::Reverse(d.last_activity_at));
    list
}

#[tauri::command]
pub fn get_discussion(id: Uuid, state: State<'_, AppState>) -> Result<DiscussionView, String> {
    state
        .fixtures
        .views
        .iter()
        .find(|v| v.discussion.id == id)
        .cloned()
        .ok_or_else(|| format!("No discussion with id {id}"))
}
