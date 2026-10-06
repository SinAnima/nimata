//! Tauri commands: thin translations between IPC and the core repository.
//! Errors cross IPC as user-readable strings.

use std::path::PathBuf;

use nimata_core::archive::DiscussionArchive;
use nimata_core::domain::{Generation, ModelParticipant, ProviderConfig, ProviderKind};
use nimata_core::providers::{Capabilities, ModelInfo, capabilities};
use nimata_core::{
    Discussion, DiscussionFilter, DiscussionSummary, DiscussionView, Participant, Post, Repository,
    Revision, Timestamp, UnixMillis,
};
use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_dialog::{DialogExt, FilePath};
use uuid::Uuid;

use crate::generation::{self, Generations};
use crate::secrets::{KeyStatus, Keys};
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderView {
    provider: ProviderConfig,
    capabilities: Capabilities,
    key: KeyStatus,
    models: Vec<ModelParticipant>,
}

/// Every provider with what it can do, its key status, and its models.
/// The standard OpenAI and Anthropic providers always exist.
#[tauri::command]
pub fn providers(
    state: State<'_, AppState>,
    keys: State<'_, Keys>,
) -> CommandResult<Vec<ProviderView>> {
    let mut repo = state.repo()?;
    for kind in [ProviderKind::OpenAi, ProviderKind::Anthropic] {
        repo.standard_provider(kind, UnixMillis::now())
            .map_err(text)?;
    }
    let models = repo.model_participants().map_err(text)?;
    repo.providers()
        .map_err(text)?
        .into_iter()
        .map(|provider| {
            Ok(ProviderView {
                capabilities: capabilities(provider.kind),
                key: keys.status(&provider)?,
                models: models
                    .iter()
                    .filter(|m| m.provider_id == provider.id)
                    .cloned()
                    .collect(),
                provider,
            })
        })
        .collect()
}

/// Adds an OpenAI-compatible connection, such as a local Ollama server.
#[tauri::command]
pub fn add_endpoint(
    display_name: String,
    base_url: String,
    key: Option<String>,
    state: State<'_, AppState>,
    keys: State<'_, Keys>,
) -> CommandResult<ProviderConfig> {
    let provider = state
        .repo()?
        .add_provider(
            ProviderKind::OpenAiCompatible,
            &display_name,
            &base_url,
            UnixMillis::now(),
        )
        .map_err(text)?;
    if let Some(key) = key.filter(|k| !k.trim().is_empty()) {
        keys.save(&provider, &key)?;
    }
    Ok(provider)
}

#[tauri::command]
pub fn update_endpoint(
    provider_id: Uuid,
    display_name: String,
    base_url: String,
    state: State<'_, AppState>,
) -> CommandResult<ProviderConfig> {
    let mut repo = state.repo()?;
    repo.rename_provider(provider_id, &display_name, UnixMillis::now())
        .map_err(text)?;
    if base_url.trim().is_empty() {
        return Err("an OpenAI-compatible connection needs its endpoint address".into());
    }
    repo.set_provider_base_url(provider_id, Some(&base_url), UnixMillis::now())
        .map_err(text)
}

/// Removes a connection whose models have written nothing, and its key.
#[tauri::command]
pub fn remove_endpoint(
    provider_id: Uuid,
    state: State<'_, AppState>,
    keys: State<'_, Keys>,
) -> CommandResult<()> {
    let config = provider_config(&state, provider_id)?;
    state.repo()?.remove_provider(provider_id).map_err(text)?;
    keys.remove(&config)?;
    Ok(())
}

fn provider_config(state: &AppState, id: Uuid) -> CommandResult<ProviderConfig> {
    state.repo()?.provider(id).map_err(text)
}

#[tauri::command]
pub fn save_api_key(
    provider_id: Uuid,
    key: String,
    state: State<'_, AppState>,
    keys: State<'_, Keys>,
) -> CommandResult<KeyStatus> {
    keys.save(&provider_config(&state, provider_id)?, &key)
}

#[tauri::command]
pub fn remove_api_key(
    provider_id: Uuid,
    state: State<'_, AppState>,
    keys: State<'_, Keys>,
) -> CommandResult<KeyStatus> {
    keys.remove(&provider_config(&state, provider_id)?)
}

/// Lists the provider's text models, which also proves the key works.
#[tauri::command]
pub async fn provider_models(
    provider_id: Uuid,
    state: State<'_, AppState>,
    keys: State<'_, Keys>,
    generations: State<'_, Generations>,
) -> CommandResult<Vec<ModelInfo>> {
    let config = provider_config(&state, provider_id)?;
    let key = keys.resolve(&config)?.map(|(key, _)| key);
    let provider = generations.provider_for(&config, key)?;
    provider.models().await.map_err(text)
}

#[tauri::command]
pub fn set_model(
    provider_id: Uuid,
    model: String,
    display_name: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> CommandResult<ModelParticipant> {
    state
        .repo()?
        .set_model(provider_id, &model, &display_name, enabled)
        .map_err(text)
}

/// Asks a model to reply to a post. The reply streams in afterwards.
#[tauri::command]
pub fn ask_model<R: Runtime>(
    discussion_id: Uuid,
    parent_id: Uuid,
    participant_id: Uuid,
    app: AppHandle<R>,
) -> CommandResult<Post> {
    generation::ask(&app, discussion_id, parent_id, participant_id)
}

#[tauri::command]
pub fn cancel_reply(post_id: Uuid, generations: State<'_, Generations>) -> bool {
    generations.cancel(post_id)
}

/// Asks the same model again, as a new reply to the same post. The failed
/// or stopped reply stays where it is.
#[tauri::command]
pub fn retry_reply<R: Runtime>(post_id: Uuid, app: AppHandle<R>) -> CommandResult<Post> {
    let state = app.state::<AppState>();
    let (discussion_id, parent_id, participant_id) = {
        let mut repo = state.repo()?;
        let generation = repo
            .generation_for_post(post_id)
            .map_err(text)?
            .ok_or("only a model's reply can be retried")?;
        if !generation.status.is_finished() {
            return Err("this reply is still being written".into());
        }
        let post = repo.discussion_of_post(post_id).map_err(text)?;
        (
            post.0,
            post.1.ok_or("this reply has no post to answer")?,
            generation.participant_id,
        )
    };
    generation::ask(&app, discussion_id, parent_id, participant_id)
}

/// How a model reply came about: its status, error, and which posts it was
/// shown.
#[tauri::command]
pub fn reply_details(
    post_id: Uuid,
    state: State<'_, AppState>,
) -> CommandResult<Option<Generation>> {
    state.repo()?.generation_for_post(post_id).map_err(text)
}

#[tauri::command]
pub fn set_model_aliases(
    participant_id: Uuid,
    aliases: Vec<String>,
    state: State<'_, AppState>,
) -> CommandResult<ModelParticipant> {
    state
        .repo()?
        .set_model_aliases(participant_id, &aliases)
        .map_err(text)
}

/// The model that answers when neither a mention nor the thread decides.
#[tauri::command]
pub fn default_model(state: State<'_, AppState>) -> CommandResult<Option<Uuid>> {
    state.repo()?.default_model().map_err(text)
}

#[tauri::command]
pub fn set_default_model(
    participant_id: Option<Uuid>,
    state: State<'_, AppState>,
) -> CommandResult<()> {
    state
        .repo()?
        .set_default_model(participant_id)
        .map_err(text)
}
