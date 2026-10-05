//! Calls commands through Tauri's IPC layer with the same JSON the UI sends,
//! so argument names and serialization are checked end to end.

use nimata_core::SqliteRepository;
use serde_json::{Value, json};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{
    INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
};
use tauri::webview::{InvokeRequest, WebviewWindow, WebviewWindowBuilder};

use crate::state::AppState;

fn app() -> WebviewWindow<MockRuntime> {
    let app = super::with_commands(mock_builder())
        .manage(AppState::with_repo(
            SqliteRepository::open_in_memory().unwrap(),
        ))
        .build(mock_context(noop_assets()))
        .unwrap();
    WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap()
}

fn invoke(webview: &WebviewWindow<MockRuntime>, cmd: &str, args: Value) -> Result<Value, Value> {
    get_ipc_response(
        webview,
        InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: "tauri://localhost".parse().unwrap(),
            body: InvokeBody::Json(args),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.to_string(),
        },
    )
    .map(|body| body.deserialize::<Value>().unwrap())
}

#[test]
fn the_ui_argument_shapes_reach_the_repository() {
    let w = app();

    let view = invoke(
        &w,
        "start_discussion",
        json!({ "title": "", "body": "Should Nimata use CouchDB?" }),
    )
    .unwrap();
    let discussion_id = view["discussion"]["id"].clone();
    let root_id = view["posts"][0]["id"].clone();
    assert_eq!(view["discussion"]["title"], "Should Nimata use CouchDB?");

    let reply = invoke(
        &w,
        "add_post",
        json!({ "discussionId": discussion_id, "parentId": root_id, "body": "The attraction is replication." }),
    )
    .unwrap();
    assert_eq!(reply["parentId"], root_id);
    assert!(reply["createdAt"].is_i64());
    assert!(reply["tzOffsetMinutes"].is_i64());

    invoke(
        &w,
        "save_draft",
        json!({ "discussionId": discussion_id, "parentId": reply["id"], "body": "But mobile" }),
    )
    .unwrap();
    let view = invoke(&w, "get_discussion", json!({ "id": discussion_id })).unwrap();
    assert_eq!(view["posts"].as_array().unwrap().len(), 2);
    assert_eq!(view["draft"]["body"], "But mobile");
    assert_eq!(view["draft"]["parentId"], reply["id"]);

    invoke(
        &w,
        "rename_discussion",
        json!({ "id": discussion_id, "title": "CouchDB" }),
    )
    .unwrap();
    invoke(
        &w,
        "set_archived",
        json!({ "id": discussion_id, "archived": true }),
    )
    .unwrap();
    let archived = invoke(&w, "list_discussions", json!({ "filter": "archived" })).unwrap();
    assert_eq!(archived[0]["title"], "CouchDB");
    assert_eq!(archived[0]["postCount"], 2);

    let me = invoke(&w, "rename_local_user", json!({ "name": "Thanos" })).unwrap();
    assert_eq!(me["displayName"], "Thanos");
}

#[test]
fn domain_errors_arrive_as_readable_messages() {
    let w = app();
    let error = invoke(
        &w,
        "start_discussion",
        json!({ "title": "x", "body": "   " }),
    )
    .unwrap_err();
    assert_eq!(error, json!("a post cannot be empty"));
}

#[test]
fn app_info_and_local_user_are_available() {
    let w = app();
    let info = invoke(&w, "app_info", json!({})).unwrap();
    assert_eq!(info["tauriVersion"], tauri::VERSION);
    assert!(info["version"].is_string());

    let me = invoke(&w, "local_user", json!({})).unwrap();
    assert_eq!(me["kind"], "human");
    assert_eq!(invoke(&w, "local_user", json!({})).unwrap()["id"], me["id"]);
}

#[test]
fn unknown_ids_and_malformed_arguments_are_rejected() {
    let w = app();
    let missing = invoke(&w, "get_discussion", json!({ "id": uuid::Uuid::now_v7() }));
    assert_eq!(missing.unwrap_err(), json!("discussion not found"));

    let bad_id = invoke(&w, "get_discussion", json!({ "id": "not-a-uuid" }));
    assert!(bad_id.is_err());

    let bad_filter = invoke(&w, "list_discussions", json!({ "filter": "everything" }));
    assert!(bad_filter.is_err());
}

#[test]
fn edits_keep_history_and_deletes_leave_tombstones() {
    let w = app();
    let view = invoke(
        &w,
        "start_discussion",
        json!({ "title": "T", "body": "first" }),
    )
    .unwrap();
    let discussion_id = view["discussion"]["id"].clone();
    let post_id = view["posts"][0]["id"].clone();

    let edited = invoke(
        &w,
        "edit_post",
        json!({ "postId": post_id, "body": "second" }),
    )
    .unwrap();
    assert_eq!(edited["body"], "second");
    assert!(edited["editedAt"].is_i64());

    let revisions = invoke(&w, "post_revisions", json!({ "postId": post_id })).unwrap();
    assert_eq!(revisions[0]["body"], "first");
    assert!(revisions[0]["writtenAt"].is_i64());
    assert!(revisions[0]["replacedAt"].is_i64());

    let deleted = invoke(&w, "delete_post", json!({ "postId": post_id })).unwrap();
    assert_eq!(deleted["body"], "");
    assert!(deleted["deletedAt"].is_i64());

    invoke(&w, "delete_discussion", json!({ "id": discussion_id })).unwrap();
    let missing = invoke(&w, "get_discussion", json!({ "id": discussion_id }));
    assert_eq!(missing.unwrap_err(), json!("discussion not found"));
}

#[test]
fn database_status_reports_an_open_database() {
    let w = app();
    let status = invoke(&w, "database_status", json!({})).unwrap();
    assert_eq!(status["error"], Value::Null);
}

#[test]
fn exported_json_is_the_canonical_archive() {
    use nimata_core::archive::DiscussionArchive;
    use tauri::Manager;

    let w = app();
    let view = invoke(
        &w,
        "start_discussion",
        json!({ "title": "T", "body": "first" }),
    )
    .unwrap();
    let post_id = view["posts"][0]["id"].clone();
    invoke(
        &w,
        "edit_post",
        json!({ "postId": post_id, "body": "second" }),
    )
    .unwrap();

    let id = uuid::Uuid::parse_str(view["discussion"]["id"].as_str().unwrap()).unwrap();
    let json = crate::commands::discussion_json(&w.state::<AppState>(), id).unwrap();
    let archive = DiscussionArchive::from_json(&json).unwrap();
    assert_eq!(archive.posts[0].body, "second");
    assert_eq!(archive.posts[0].revisions[0].body, "first");
}
