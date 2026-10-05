//! Tauri commands: thin translations between IPC and the core repository.
//! Errors cross IPC as user-readable strings.

use std::path::PathBuf;

use nimata_core::archive::DiscussionArchive;
use nimata_core::{
    Discussion, DiscussionFilter, DiscussionSummary, DiscussionView, Participant, Post, Repository,
    Revision, Timestamp, UnixMillis,
};
use serde::Serialize;
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_dialog::{DialogExt, FilePath};
use uuid::Uuid;

use crate::state::{AppState, DatabaseStatus};

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
        database_path: state.database_path().map(|p| p.display().to_string()),
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
    state
        .repo()?
        .rename_discussion(id, &title, UnixMillis::now())
        .map_err(text)
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

/// Edits a post by the local user, keeping the previous text as a revision.
#[tauri::command]
pub fn edit_post(post_id: Uuid, body: String, state: State<'_, AppState>) -> CommandResult<Post> {
    let mut repo = state.repo()?;
    let me = repo.local_user().map_err(text)?;
    repo.edit_post(post_id, me.id, &body, UnixMillis::now())
        .map_err(text)
}

#[tauri::command]
pub fn post_revisions(post_id: Uuid, state: State<'_, AppState>) -> CommandResult<Vec<Revision>> {
    state.repo()?.post_revisions(post_id).map_err(text)
}

/// Deletes a post by the local user, leaving a tombstone in the thread.
#[tauri::command]
pub fn delete_post(post_id: Uuid, state: State<'_, AppState>) -> CommandResult<Post> {
    let mut repo = state.repo()?;
    let me = repo.local_user().map_err(text)?;
    repo.delete_post(post_id, me.id, UnixMillis::now())
        .map_err(text)
}

#[tauri::command]
pub fn delete_discussion(id: Uuid, state: State<'_, AppState>) -> CommandResult<()> {
    state
        .repo()?
        .delete_discussion(id, UnixMillis::now())
        .map_err(text)
}

#[tauri::command]
pub fn database_status(state: State<'_, AppState>) -> DatabaseStatus {
    state.status()
}

/// The canonical JSON (`nimata/1`) for one discussion.
pub fn discussion_json(state: &AppState, id: Uuid) -> CommandResult<String> {
    let mut repo = state.repo()?;
    let view = repo.get_discussion(id).map_err(text)?;
    let revisions = repo.discussion_revisions(id).map_err(text)?;
    Ok(DiscussionArchive::new(&view, &revisions, UnixMillis::now()).to_json())
}

fn local_path(file: FilePath) -> CommandResult<PathBuf> {
    file.into_path()
        .map_err(|_| "this location cannot be used; choose a folder on this device".to_string())
}

/// A file name that is safe on every platform, from a discussion title.
fn file_name_from(title: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' {
                c
            } else {
                ' '
            }
        })
        .collect();
    let words: Vec<&str> = cleaned.split_whitespace().take(8).collect();
    if words.is_empty() {
        "discussion".into()
    } else {
        words.join(" ")
    }
}

fn today() -> String {
    nimata_core::archive::format_utc(UnixMillis::now())[..10].to_string()
}

/// Asks where to save, then writes the discussion as canonical JSON.
/// Returns the chosen path, or nothing if the user cancelled.
#[tauri::command]
pub async fn export_discussion<R: Runtime>(
    id: Uuid,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> CommandResult<Option<String>> {
    let json = discussion_json(&state, id)?;
    let title = state
        .repo()?
        .get_discussion(id)
        .map_err(text)?
        .discussion
        .title;
    let Some(file) = app
        .dialog()
        .file()
        .set_title("Export discussion")
        .set_file_name(format!("{}.nimata.json", file_name_from(&title)))
        .add_filter("Nimata discussion", &["json"])
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = local_path(file)?;
    std::fs::write(&path, json).map_err(text)?;
    Ok(Some(path.display().to_string()))
}

/// Asks where to save, then writes a complete backup of the database.
#[tauri::command]
pub async fn backup_database<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> CommandResult<Option<String>> {
    let Some(file) = app
        .dialog()
        .file()
        .set_title("Back up Nimata")
        .set_file_name(format!("nimata-backup-{}.sqlite3", today()))
        .add_filter("Nimata backup", &["sqlite3"])
        .blocking_save_file()
    else {
        return Ok(None);
    };
    let path = local_path(file)?;
    state.backup_to(&path)?;
    Ok(Some(path.display().to_string()))
}

/// Asks for a backup file, then replaces the database with it.
#[tauri::command]
pub async fn restore_database<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> CommandResult<Option<String>> {
    let Some(file) = app
        .dialog()
        .file()
        .set_title("Restore from backup")
        .add_filter("Nimata backup", &["sqlite3", "db"])
        .blocking_pick_file()
    else {
        return Ok(None);
    };
    let path = local_path(file)?;
    state.restore_from(&path)?;
    Ok(Some(path.display().to_string()))
}

#[cfg(test)]
mod tests {
    use super::file_name_from;

    #[test]
    fn export_file_names_are_safe_everywhere() {
        assert_eq!(
            file_name_from("Should Nimata use CouchDB?"),
            "Should Nimata use CouchDB"
        );
        assert_eq!(file_name_from("a/b\\c:d*e"), "a b c d e");
        assert_eq!(file_name_from("???"), "discussion");
        assert_eq!(file_name_from("νήματα: threads"), "νήματα threads");
    }
}
