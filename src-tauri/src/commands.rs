//! Tauri commands: thin translations between IPC and the core repository.
//! Errors cross IPC as user-readable strings.

use nimata_core::{
    Discussion, DiscussionFilter, DiscussionSummary, DiscussionView, Participant, Post, Repository,
    Timestamp, UnixMillis,
};
use serde::Serialize;
use tauri::{AppHandle, Runtime, State};
use uuid::Uuid;

use crate::state::AppState;

type CommandResult<T> = Result<T, String>;

fn text<E: ToString>(e: E) -> String {
    e.to_string()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    tauri_version: &'static str,
    database_path: Option<String>,
    platform: &'static str,
}

#[tauri::command]
pub fn app_info<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        tauri_version: tauri::VERSION,
        database_path: state
            .database_path
            .as_ref()
            .map(|p| p.display().to_string()),
        platform: std::env::consts::OS,
    }
}

#[tauri::command]
pub fn local_user(state: State<'_, AppState>) -> CommandResult<Participant> {
    state.repo()?.local_user().map_err(text)
}

#[tauri::command]
pub fn rename_local_user(name: String, state: State<'_, AppState>) -> CommandResult<Participant> {
    let mut repo = state.repo()?;
    let me = repo.local_user().map_err(text)?;
    repo.rename_participant(me.id, &name).map_err(text)
}

#[tauri::command]
pub fn list_discussions(
    filter: DiscussionFilter,
    state: State<'_, AppState>,
) -> CommandResult<Vec<DiscussionSummary>> {
    state.repo()?.list_discussions(filter).map_err(text)
}

#[tauri::command]
pub fn get_discussion(id: Uuid, state: State<'_, AppState>) -> CommandResult<DiscussionView> {
    state.repo()?.get_discussion(id).map_err(text)
}

/// Starts a discussion as the local user and returns it ready to display.
#[tauri::command]
pub fn start_discussion(
    title: String,
    body: String,
    state: State<'_, AppState>,
) -> CommandResult<DiscussionView> {
    let mut repo = state.repo()?;
    let me = repo.local_user().map_err(text)?;
    let (discussion, _) = repo
        .start_discussion(&title, me.id, &body, Timestamp::now())
        .map_err(text)?;
    repo.get_discussion(discussion.id).map_err(text)
}

/// Adds a post by the local user.
#[tauri::command]
pub fn add_post(
    discussion_id: Uuid,
    parent_id: Option<Uuid>,
    body: String,
    state: State<'_, AppState>,
) -> CommandResult<Post> {
    let mut repo = state.repo()?;
    let me = repo.local_user().map_err(text)?;
    repo.add_post(discussion_id, parent_id, me.id, &body, Timestamp::now())
        .map_err(text)
}

#[tauri::command]
pub fn rename_discussion(
    id: Uuid,
    title: String,
    state: State<'_, AppState>,
) -> CommandResult<Discussion> {
    state.repo()?.rename_discussion(id, &title).map_err(text)
}

#[tauri::command]
pub fn set_archived(
    id: Uuid,
    archived: bool,
    state: State<'_, AppState>,
) -> CommandResult<Discussion> {
    state
        .repo()?
        .set_archived(id, archived, UnixMillis::now())
        .map_err(text)
}

#[tauri::command]
pub fn save_draft(
    discussion_id: Uuid,
    parent_id: Option<Uuid>,
    body: String,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    state
        .repo()?
        .save_draft(discussion_id, parent_id, &body, UnixMillis::now())
        .map(|_| ())
        .map_err(text)
}
