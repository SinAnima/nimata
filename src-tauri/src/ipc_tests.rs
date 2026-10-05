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
