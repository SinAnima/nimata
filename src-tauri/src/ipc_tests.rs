//! Calls commands through Tauri's IPC layer with the same JSON the UI sends,
//! so argument names and serialization are checked end to end.

use nimata_core::SqliteRepository;
use serde_json::{Value, json};
use tauri::ipc::{CallbackFn, InvokeBody};
use tauri::test::{
    INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder, mock_context, noop_assets,
};
use tauri::webview::{InvokeRequest, WebviewWindow, WebviewWindowBuilder};

use crate::generation::Generations;
use crate::secrets::{Keys, MemorySecretStore};
use crate::state::AppState;

fn app() -> WebviewWindow<MockRuntime> {
    let app = super::with_commands(mock_builder())
        .manage(AppState::with_repo(
            SqliteRepository::open_in_memory().unwrap(),
        ))
        .manage(Keys::new(
            Box::new(MemorySecretStore::default()),
            true,
            |_| None,
        ))
        .manage(Generations::default())
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
fn search_finds_posts_and_remembers_queries() {
    let w = app();
    invoke(
        &w,
        "start_discussion",
        json!({ "title": "Νήματα", "body": "Could Datalog replace the Oracle rules?" }),
    )
    .unwrap();
    let results = invoke(&w, "search", json!({ "query": "datal from:me" })).unwrap();
    assert_eq!(results["posts"].as_array().unwrap().len(), 1);
    assert_eq!(results["posts"][0]["discussionTitle"], "Νήματα");
    assert_eq!(
        results["posts"][0]["snippet"],
        "Could \u{E000}Datalog\u{E001} replace the Oracle rules?"
    );
    assert_eq!(results["highlight"], json!(["datal"]));
    let titles = invoke(&w, "search", json!({ "query": "νηματα" })).unwrap();
    assert_eq!(titles["discussions"][0]["title"], "\u{E000}Νήματα\u{E001}");

    let warned = invoke(&w, "search", json!({ "query": "after:soon" })).unwrap();
    assert_eq!(warned["warnings"].as_array().unwrap().len(), 1);

    invoke(&w, "record_search", json!({ "query": "datalog" })).unwrap();
    let recent = invoke(&w, "record_search", json!({ "query": "oracle" })).unwrap();
    assert_eq!(recent, json!(["oracle", "datalog"]));
    invoke(&w, "clear_recent_searches", json!({})).unwrap();
    assert_eq!(invoke(&w, "recent_searches", json!({})).unwrap(), json!([]));
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

mod replies {
    //! Model replies end to end: IPC commands, the generation runner, the
    //! OpenAI adapter, and the database, against a local server that
    //! replays a recorded OpenAI stream.

    use super::*;
    use nimata_core::Repository;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::time::{Duration, Instant};
    use tauri::Manager;

    const RECORDED: &str =
        include_str!("../../crates/nimata-core/tests/fixtures/openai_responses_stream.sse");

    struct Reply {
        status: &'static str,
        body: String,
        /// Pause before closing, to keep a stream open.
        hold: Duration,
    }

    /// Serves each reply to one connection, in order, on a background thread.
    fn serve(replies: Vec<Reply>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}/v1", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for reply in replies {
                let Ok((mut socket, _)) = listener.accept() else {
                    return;
                };
                let mut request = Vec::new();
                let mut buf = [0u8; 8192];
                loop {
                    let n = socket.read(&mut buf).unwrap_or(0);
                    request.extend_from_slice(&buf[..n]);
                    let text = String::from_utf8_lossy(&request);
                    if let Some(end) = text.find("\r\n\r\n") {
                        let length = text[..end]
                            .lines()
                            .find_map(|l| {
                                l.to_ascii_lowercase()
                                    .strip_prefix("content-length:")
                                    .map(|v| v.trim().parse::<usize>().unwrap())
                            })
                            .unwrap_or(0);
                        if request.len() >= end + 4 + length {
                            break;
                        }
                    }
                    if n == 0 {
                        break;
                    }
                }
                let content_type = if reply.status.starts_with("200") {
                    "text/event-stream"
                } else {
                    "application/json"
                };
                let head = format!(
                    "HTTP/1.1 {}\r\nContent-Type: {content_type}\r\nx-request-id: req_test\r\nConnection: close\r\n\r\n",
                    reply.status
                );
                let _ = socket.write_all(head.as_bytes());
                for chunk in reply.body.as_bytes().chunks(64) {
                    let _ = socket.write_all(chunk);
                    let _ = socket.flush();
                    std::thread::sleep(Duration::from_millis(1));
                }
                std::thread::sleep(reply.hold);
            }
        });
        base
    }

    struct Setup {
        w: WebviewWindow<MockRuntime>,
        discussion: Value,
        question: Value,
        model: Value,
    }

    /// A discussion with one question, OpenAI pointed at `base`, a key
    /// saved, and GPT-5.6 enabled.
    fn setup(base: &str) -> Setup {
        let w = app();
        let providers = invoke(&w, "providers", json!({})).unwrap();
        let provider_id = providers[0]["provider"]["id"].clone();
        {
            let state = w.state::<AppState>();
            let id = uuid::Uuid::parse_str(provider_id.as_str().unwrap()).unwrap();
            state
                .repo()
                .unwrap()
                .set_provider_base_url(id, Some(base), nimata_core::UnixMillis(0))
                .unwrap();
        }
        invoke(
            &w,
            "save_api_key",
            json!({ "providerId": provider_id, "key": "sk-test-1234" }),
        )
        .unwrap();
        let model = invoke(
            &w,
            "set_model",
            json!({ "providerId": provider_id, "model": "gpt-5.6", "displayName": "GPT-5.6", "enabled": true }),
        )
        .unwrap();
        let view = invoke(
            &w,
            "start_discussion",
            json!({ "title": "", "body": "Should Nimata use CouchDB?" }),
        )
        .unwrap();
        Setup {
            discussion: view["discussion"]["id"].clone(),
            question: view["posts"][0]["id"].clone(),
            model: model["participant"]["id"].clone(),
            w,
        }
    }

    fn ask(s: &Setup, parent: &Value) -> Result<Value, Value> {
        invoke(
            &s.w,
            "ask_model",
            json!({ "discussionId": s.discussion, "parentId": parent, "participantId": s.model }),
        )
    }

    /// Waits until the reply in `post_id` has finished, and returns its record.
    fn finished(s: &Setup, post_id: &Value) -> Value {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            let details = invoke(&s.w, "reply_details", json!({ "postId": post_id })).unwrap();
            let status = details["status"].as_str().unwrap_or_default();
            if ["complete", "failed", "cancelled"].contains(&status) {
                return details;
            }
            assert!(Instant::now() < deadline, "reply did not finish: {details}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn post(s: &Setup, id: &Value) -> Value {
        let view = invoke(&s.w, "get_discussion", json!({ "id": s.discussion })).unwrap();
        view["posts"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == *id)
            .unwrap()
            .clone()
    }

    #[test]
    fn a_model_reply_streams_into_a_post() {
        let base = serve(vec![Reply {
            status: "200 OK",
            body: RECORDED.into(),
            hold: Duration::ZERO,
        }]);
        let s = setup(&base);

        let reply = ask(&s, &s.question).unwrap();
        assert_eq!(reply["status"], "streaming");
        assert_eq!(reply["parentId"], s.question);

        let details = finished(&s, &reply["id"]);
        assert_eq!(details["status"], "complete");
        assert_eq!(details["contextPostIds"], json!([s.question]));

        let done = post(&s, &reply["id"]);
        assert_eq!(
            done["body"],
            "Replication is the attraction, but mobile changes the constraints."
        );
        assert_eq!(done["status"], "complete");
        assert_eq!(done["providerMetadata"]["model"], "gpt-5.6-2026-08-01");
        assert_eq!(done["providerMetadata"]["requestId"], "req_test");
        assert_eq!(done["providerMetadata"]["outputTokens"], 12);
        assert_ne!(
            done["id"], done["providerMetadata"]["responseId"],
            "provider IDs never become Nimata IDs"
        );
    }

    #[test]
    fn cancelling_keeps_the_text_received_so_far() {
        let partial = &RECORDED[..RECORDED.find("but mobile").unwrap()];
        let base = serve(vec![Reply {
            status: "200 OK",
            body: partial.into(),
            hold: Duration::from_secs(5),
        }]);
        let s = setup(&base);
        let reply = ask(&s, &s.question).unwrap();

        // Wait for some text to arrive, then stop the reply.
        let deadline = Instant::now() + Duration::from_secs(5);
        while post(&s, &reply["id"])["body"].as_str().unwrap().is_empty() {
            assert!(Instant::now() < deadline, "no text arrived");
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(
            invoke(&s.w, "cancel_reply", json!({ "postId": reply["id"] })).unwrap(),
            json!(true)
        );

        let details = finished(&s, &reply["id"]);
        assert_eq!(details["status"], "cancelled");
        let stopped = post(&s, &reply["id"]);
        assert_eq!(stopped["status"], "cancelled");
        assert!(
            stopped["body"].as_str().unwrap().starts_with("Replication"),
            "{stopped}"
        );
    }

    #[test]
    fn a_rejected_key_fails_with_an_explanation_and_can_be_retried() {
        let base = serve(vec![
            Reply {
                status: "401 Unauthorized",
                body:
                    r#"{"error":{"message":"Incorrect API key provided","code":"invalid_api_key"}}"#
                        .into(),
                hold: Duration::ZERO,
            },
            Reply {
                status: "200 OK",
                body: RECORDED.into(),
                hold: Duration::ZERO,
            },
        ]);
        let s = setup(&base);
        let reply = ask(&s, &s.question).unwrap();
        let details = finished(&s, &reply["id"]);
        assert_eq!(details["status"], "failed");
        assert!(
            details["error"]
                .as_str()
                .unwrap()
                .contains("rejected the API key"),
            "{details}"
        );
        assert_eq!(post(&s, &reply["id"])["status"], "failed");
        assert!(
            details["error"]
                .as_str()
                .unwrap()
                .contains("Incorrect API key provided"),
            "{details}"
        );
        assert_eq!(
            post(&s, &reply["id"])["providerMetadata"]["requestId"],
            "req_test"
        );

        let retried = invoke(&s.w, "retry_reply", json!({ "postId": reply["id"] })).unwrap();
        assert_ne!(retried["id"], reply["id"], "a retry is a new reply");
        assert_eq!(retried["parentId"], s.question, "to the same post");
        assert_eq!(finished(&s, &retried["id"])["status"], "complete");
        assert_eq!(
            post(&s, &reply["id"])["status"],
            "failed",
            "the failed reply stays"
        );
    }

    #[test]
    fn an_error_inside_the_stream_reaches_the_person_with_its_reason() {
        let stream = concat!(
            "event: response.created\n",
            "data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_1\"}}\n\n",
            "event: error\n",
            "data: {\"type\":\"error\",\"error\":{\"type\":\"invalid_request_error\",",
            "\"code\":\"unsupported_value\",\"message\":\"Streaming is not supported for this model\"}}\n\n",
        );
        let base = serve(vec![Reply {
            status: "200 OK",
            body: stream.into(),
            hold: Duration::ZERO,
        }]);
        let s = setup(&base);
        let reply = ask(&s, &s.question).unwrap();
        let details = finished(&s, &reply["id"]);
        assert_eq!(details["status"], "failed");
        assert_eq!(
            details["error"],
            "OpenAI reported an error while replying. (Streaming is not supported for this model [unsupported_value])"
        );
        assert_eq!(
            post(&s, &reply["id"])["providerMetadata"]["requestId"],
            "req_test"
        );
    }

    #[test]
    fn aliases_and_the_default_model_round_trip() {
        let base = serve(vec![]);
        let s = setup(&base);
        let model = invoke(
            &s.w,
            "set_model_aliases",
            json!({ "participantId": s.model, "aliases": ["@Review", "r"] }),
        )
        .unwrap();
        assert_eq!(model["aliases"], json!(["review", "r"]));
        let providers = invoke(&s.w, "providers", json!({})).unwrap();
        assert_eq!(providers[0]["models"][0]["aliases"], json!(["review", "r"]));

        assert_eq!(
            invoke(&s.w, "default_model", json!({})).unwrap(),
            Value::Null
        );
        invoke(
            &s.w,
            "set_default_model",
            json!({ "participantId": s.model }),
        )
        .unwrap();
        assert_eq!(invoke(&s.w, "default_model", json!({})).unwrap(), s.model);
        invoke(&s.w, "set_default_model", json!({ "participantId": null })).unwrap();
        assert_eq!(
            invoke(&s.w, "default_model", json!({})).unwrap(),
            Value::Null
        );
    }

    const COMPATIBLE: &str =
        include_str!("../../crates/nimata-core/tests/fixtures/openai_compatible_stream.sse");

    #[test]
    fn the_standard_providers_are_listed_with_their_capabilities() {
        let w = app();
        let providers = invoke(&w, "providers", json!({})).unwrap();
        let kinds: Vec<_> = providers
            .as_array()
            .unwrap()
            .iter()
            .map(|p| p["provider"]["kind"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(kinds, vec!["openai", "anthropic"]);
        assert_eq!(providers[1]["capabilities"]["requiresKey"], true);
        assert_eq!(
            providers[1]["key"]["environmentVariable"],
            "ANTHROPIC_API_KEY"
        );
    }

    /// Adds an OpenAI-compatible connection at `base` with one enabled model.
    fn local_model(s: &Setup, base: &str) -> (Value, Value) {
        let endpoint = invoke(
            &s.w,
            "add_endpoint",
            json!({ "displayName": "Ollama", "baseUrl": base, "key": null }),
        )
        .unwrap();
        let model = invoke(
            &s.w,
            "set_model",
            json!({ "providerId": endpoint["id"], "model": "qwen3:8b", "displayName": "Qwen", "enabled": true }),
        )
        .unwrap();
        (endpoint, model["participant"]["id"].clone())
    }

    #[test]
    fn a_local_model_replies_without_a_key() {
        let s = setup(&serve(vec![]));
        let local = serve(vec![Reply {
            status: "200 OK",
            body: COMPATIBLE.into(),
            hold: Duration::ZERO,
        }]);
        let (_, qwen) = local_model(&s, &local);

        let reply = invoke(
            &s.w,
            "ask_model",
            json!({ "discussionId": s.discussion, "parentId": s.question, "participantId": qwen }),
        )
        .unwrap();
        assert_eq!(finished(&s, &reply["id"])["status"], "complete");
        let done = post(&s, &reply["id"]);
        assert_eq!(done["body"], "Local models can join too.");
        assert_eq!(done["providerMetadata"]["provider"], "openai_compatible");
        assert_eq!(done["providerMetadata"]["model"], "qwen3:8b");
    }

    #[test]
    fn one_provider_failing_leaves_the_other_reply_intact() {
        let openai = serve(vec![Reply {
            status: "500 Internal Server Error",
            body: r#"{"error":{"message":"boom"}}"#.into(),
            hold: Duration::ZERO,
        }]);
        let s = setup(&openai);
        let local = serve(vec![Reply {
            status: "200 OK",
            body: COMPATIBLE.into(),
            hold: Duration::from_millis(200),
        }]);
        let (_, qwen) = local_model(&s, &local);

        // Asked at the same moment, as sibling replies to the question.
        let gpt_reply = ask(&s, &s.question).unwrap();
        let qwen_reply = invoke(
            &s.w,
            "ask_model",
            json!({ "discussionId": s.discussion, "parentId": s.question, "participantId": qwen }),
        )
        .unwrap();

        assert_eq!(finished(&s, &gpt_reply["id"])["status"], "failed");
        assert_eq!(finished(&s, &qwen_reply["id"])["status"], "complete");
        assert_eq!(
            post(&s, &qwen_reply["id"])["body"],
            "Local models can join too."
        );
        assert_eq!(post(&s, &gpt_reply["id"])["parentId"], s.question);
        assert_eq!(post(&s, &qwen_reply["id"])["parentId"], s.question);
    }

    #[test]
    fn an_unused_connection_can_be_removed() {
        let s = setup(&serve(vec![]));
        let (endpoint, _) = local_model(&s, "http://127.0.0.1:9/v1");
        invoke(
            &s.w,
            "remove_endpoint",
            json!({ "providerId": endpoint["id"] }),
        )
        .unwrap();
        let providers = invoke(&s.w, "providers", json!({})).unwrap();
        assert_eq!(providers.as_array().unwrap().len(), 2);
    }

    /// The Stage 5 demo: reply to one branch while including its sibling as
    /// context, preview what the model will get, then check it got exactly
    /// that.
    #[test]
    fn the_preview_matches_what_the_model_is_sent() {
        let base = serve(vec![Reply {
            status: "200 OK",
            body: RECORDED.into(),
            hold: Duration::ZERO,
        }]);
        let s = setup(&base);
        let add = |parent: &Value, body: &str, context: Value| {
            invoke(
                &s.w,
                "add_post",
                json!({ "discussionId": s.discussion, "parentId": parent, "body": body, "contextIds": context }),
            )
            .unwrap()
        };
        let left = add(&s.question, "Yes: replication.", json!([]));
        let right = add(&s.question, "No: mobile changes it.", json!([]));

        let preview = invoke(
            &s.w,
            "preview_context",
            json!({
                "discussionId": s.discussion, "parentId": left["id"], "participantId": s.model,
                "draft": "Weigh both.", "contextIds": [right["id"]]
            }),
        )
        .unwrap();
        let summary = |sent: &Value| -> Vec<(String, String)> {
            sent["messages"]
                .as_array()
                .unwrap()
                .iter()
                .map(|m| {
                    (
                        m["source"].as_str().unwrap().into(),
                        m["text"].as_str().unwrap().into(),
                    )
                })
                .collect()
        };
        let expected = vec![
            (
                "thread".to_string(),
                "Me:\nShould Nimata use CouchDB?".to_string(),
            ),
            ("thread".to_string(), "Me:\nYes: replication.".to_string()),
            (
                "context".to_string(),
                format!(
                    "{}\n\nMe wrote:\nNo: mobile changes it.",
                    nimata_core::context::CONTEXT_LABEL
                ),
            ),
            ("thread".to_string(), "Me:\nWeigh both.".to_string()),
        ];
        assert_eq!(summary(&preview), expected);
        assert!(preview["estimatedTokens"].as_u64().unwrap() > 0);

        let mine = add(&left["id"], "Weigh both.", json!([right["id"]]));
        assert_eq!(mine["contextIds"], json!([right["id"]]));
        let reply = ask(&s, &mine["id"]).unwrap();
        let details = finished(&s, &reply["id"]);
        assert_eq!(details["status"], "complete");
        assert_eq!(
            summary(&details["sent"]),
            expected,
            "the model got what the preview showed"
        );
        assert_eq!(details["sent"]["model"], "gpt-5.6");
    }

    #[test]
    fn asking_without_a_key_explains_what_to_do() {
        let base = serve(vec![]);
        let s = setup(&base);
        let providers = invoke(&s.w, "providers", json!({})).unwrap();
        invoke(
            &s.w,
            "remove_api_key",
            json!({ "providerId": providers[0]["provider"]["id"] }),
        )
        .unwrap();

        let error = ask(&s, &s.question).unwrap_err();
        assert_eq!(error, json!("Add an OpenAI API key in Settings, Models."));
        let view = invoke(&s.w, "get_discussion", json!({ "id": s.discussion })).unwrap();
        assert_eq!(
            view["posts"].as_array().unwrap().len(),
            1,
            "nothing was created"
        );
    }

    #[test]
    fn the_settings_overview_never_contains_the_key() {
        let base = serve(vec![]);
        let s = setup(&base);
        let providers = invoke(&s.w, "providers", json!({})).unwrap();
        let text = providers.to_string();
        assert!(!text.contains("sk-test-1234"), "{text}");
        assert_eq!(providers[0]["key"]["source"], "saved");
        assert_eq!(
            providers[0]["models"][0]["participant"]["displayName"],
            "GPT-5.6"
        );
    }
}
