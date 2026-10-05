//! Model participants and the durable lifecycle of a model reply.

use nimata_core::domain::{GenerationStatus, PostStatus, ProviderKind, ProviderMetadata};
use nimata_core::repository::{GenerationOutcome, INTERRUPTED};
use nimata_core::{Error, Repository, SqliteRepository, Timestamp, UnixMillis};
use tempfile::TempDir;
use uuid::Uuid;

const T0: i64 = 1_791_132_067_123;

fn at(minutes: i64) -> Timestamp {
    Timestamp {
        at: UnixMillis(T0 + minutes * 60_000),
        offset_minutes: -240,
    }
}

struct Setup {
    repo: SqliteRepository,
    me: Uuid,
    gpt: Uuid,
    discussion: Uuid,
    question: Uuid,
}

fn setup() -> Setup {
    let mut repo = SqliteRepository::open_in_memory().unwrap();
    let me = repo.local_user().unwrap().id;
    let openai = repo
        .standard_provider(ProviderKind::OpenAi, UnixMillis(T0))
        .unwrap();
    let gpt = repo
        .set_model(openai.id, "gpt-5.6", "GPT-5.6", true)
        .unwrap()
        .participant
        .id;
    let (d, q) = repo
        .start_discussion("", me, "Should Nimata use CouchDB?", at(0))
        .unwrap();
    Setup {
        repo,
        me,
        gpt,
        discussion: d.id,
        question: q.id,
    }
}

fn complete(body: &str) -> GenerationOutcome {
    GenerationOutcome {
        status: GenerationStatus::Complete,
        body: body.into(),
        metadata: Some(ProviderMetadata {
            provider: "openai".into(),
            model: Some("gpt-5.6-2026-08-01".into()),
            response_id: Some("resp_1".into()),
            request_id: Some("req_1".into()),
            input_tokens: Some(41),
            output_tokens: Some(12),
            incomplete_reason: None,
        }),
        error: None,
    }
}

#[test]
fn the_standard_provider_is_created_once() {
    let mut repo = SqliteRepository::open_in_memory().unwrap();
    let a = repo
        .standard_provider(ProviderKind::OpenAi, UnixMillis(1))
        .unwrap();
    let b = repo
        .standard_provider(ProviderKind::OpenAi, UnixMillis(2))
        .unwrap();
    assert_eq!(a, b);
    assert_eq!(a.display_name, "OpenAI");
    assert_eq!(a.base_url, None);
}

#[test]
fn enabling_a_model_twice_updates_one_participant() {
    let mut s = setup();
    let provider = s
        .repo
        .standard_provider(ProviderKind::OpenAi, UnixMillis(0))
        .unwrap()
        .id;
    let renamed = s.repo.set_model(provider, "gpt-5.6", "GPT", false).unwrap();
    assert_eq!(renamed.participant.id, s.gpt);
    assert_eq!(renamed.participant.display_name, "GPT");
    assert!(!renamed.enabled);
    assert_eq!(renamed.participant.model.as_deref(), Some("gpt-5.6"));
    assert_eq!(renamed.participant.provider.as_deref(), Some("openai"));

    s.repo
        .set_model(provider, "gpt-5.6-mini", "GPT mini", true)
        .unwrap();
    let names: Vec<_> = s
        .repo
        .model_participants()
        .unwrap()
        .into_iter()
        .map(|m| (m.participant.display_name, m.enabled))
        .collect();
    assert_eq!(
        names,
        vec![("GPT".into(), false), ("GPT mini".into(), true)]
    );
}

#[test]
fn a_reply_streams_into_a_post_and_finishes_with_metadata() {
    let mut s = setup();
    let (post, generation) = s
        .repo
        .begin_generation(s.discussion, s.question, s.gpt, &[s.question], at(1))
        .unwrap();
    assert_eq!(post.status, PostStatus::Streaming);
    assert_eq!(post.author_id, s.gpt);
    assert_eq!(post.parent_id, Some(s.question));
    assert_eq!(post.tz_offset_minutes, -240);
    assert_eq!(generation.status, GenerationStatus::Sending);
    assert_eq!(generation.context_post_ids, vec![s.question]);

    s.repo
        .update_generation(generation.id, GenerationStatus::Streaming, "Replication is")
        .unwrap();
    let streaming = s.repo.get_discussion(s.discussion).unwrap().posts[1].clone();
    assert_eq!(streaming.body, "Replication is");

    let done = s
        .repo
        .finish_generation(
            generation.id,
            complete("Replication is the attraction."),
            UnixMillis(T0 + 120_000),
        )
        .unwrap();
    assert_eq!(done.status, PostStatus::Complete);
    assert_eq!(done.body, "Replication is the attraction.");
    let meta = done.provider_metadata.unwrap();
    assert_eq!(meta.model.as_deref(), Some("gpt-5.6-2026-08-01"));
    assert_eq!(meta.output_tokens, Some(12));
    assert_eq!(
        done.created_at,
        at(1).at,
        "the post is dated when it was asked for"
    );

    let record = s.repo.generation_for_post(post.id).unwrap().unwrap();
    assert_eq!(record.status, GenerationStatus::Complete);
    assert_eq!(record.finished_at, Some(UnixMillis(T0 + 120_000)));

    let view = s.repo.get_discussion(s.discussion).unwrap();
    assert_eq!(
        view.participants.len(),
        2,
        "the model is listed as a participant"
    );
    assert_eq!(view.discussion.updated_at, UnixMillis(T0 + 120_000));
}

#[test]
fn failed_and_cancelled_replies_keep_the_text_received() {
    for status in [GenerationStatus::Failed, GenerationStatus::Cancelled] {
        let mut s = setup();
        let (post, generation) = s
            .repo
            .begin_generation(s.discussion, s.question, s.gpt, &[s.question], at(1))
            .unwrap();
        let ended = s
            .repo
            .finish_generation(
                generation.id,
                GenerationOutcome {
                    status,
                    body: "Half an answ".into(),
                    metadata: None,
                    error: Some("The connection ended.".into()),
                },
                UnixMillis(T0 + 1),
            )
            .unwrap();
        assert_eq!(ended.body, "Half an answ");
        let record = s.repo.generation_for_post(post.id).unwrap().unwrap();
        assert_eq!(record.status, status);
        assert_eq!(record.error.as_deref(), Some("The connection ended."));

        // A second ending, e.g. text arriving after a cancel, changes nothing.
        let again = s
            .repo
            .finish_generation(generation.id, complete("late"), UnixMillis(T0 + 2))
            .unwrap();
        assert_eq!(again.body, "Half an answ");
        assert!(
            s.repo
                .update_generation(generation.id, GenerationStatus::Streaming, "x")
                .is_err()
        );
    }
}

#[test]
fn replies_still_running_at_startup_are_marked_interrupted() {
    let dir = TempDir::new().unwrap();
    let path = dir.path().join("nimata.sqlite3");
    let (post_id, generation_id) = {
        let mut repo = SqliteRepository::open(&path).unwrap();
        let me = repo.local_user().unwrap().id;
        let provider = repo
            .standard_provider(ProviderKind::OpenAi, UnixMillis(0))
            .unwrap()
            .id;
        let gpt = repo
            .set_model(provider, "gpt-5.6", "GPT-5.6", true)
            .unwrap()
            .participant
            .id;
        let (d, q) = repo.start_discussion("", me, "Question", at(0)).unwrap();
        let (post, generation) = repo
            .begin_generation(d.id, q.id, gpt, &[q.id], at(1))
            .unwrap();
        repo.update_generation(generation.id, GenerationStatus::Streaming, "Partial")
            .unwrap();
        (post.id, generation.id)
        // Nimata "crashes" here: the reply never finishes.
    };

    let mut repo = SqliteRepository::open(&path).unwrap();
    assert_eq!(repo.recover_interrupted(UnixMillis(T0 + 5)).unwrap(), 1);
    assert_eq!(repo.recover_interrupted(UnixMillis(T0 + 6)).unwrap(), 0);

    let record = repo.generation_for_post(post_id).unwrap().unwrap();
    assert_eq!(record.id, generation_id);
    assert_eq!(record.status, GenerationStatus::Failed);
    assert_eq!(record.error.as_deref(), Some(INTERRUPTED));
    let discussion = repo
        .list_discussions(nimata_core::DiscussionFilter::Active)
        .unwrap()[0]
        .id;
    let posts = repo.get_discussion(discussion).unwrap().posts;
    let post = posts.iter().find(|p| p.id == post_id).unwrap();
    assert_eq!(
        (post.status, post.body.as_str()),
        (PostStatus::Failed, "Partial")
    );
}

#[test]
fn a_reply_needs_an_enabled_model_and_a_finished_parent() {
    let mut s = setup();
    let provider = s
        .repo
        .standard_provider(ProviderKind::OpenAi, UnixMillis(0))
        .unwrap()
        .id;

    assert!(matches!(
        s.repo
            .begin_generation(s.discussion, s.question, s.me, &[], at(1)),
        Err(Error::NotFound(_))
    ));

    s.repo
        .set_model(provider, "gpt-5.6", "GPT-5.6", false)
        .unwrap();
    assert!(matches!(
        s.repo
            .begin_generation(s.discussion, s.question, s.gpt, &[], at(1)),
        Err(Error::Invalid(_))
    ));
    s.repo
        .set_model(provider, "gpt-5.6", "GPT-5.6", true)
        .unwrap();

    let (streaming, _) = s
        .repo
        .begin_generation(s.discussion, s.question, s.gpt, &[], at(1))
        .unwrap();
    assert!(matches!(
        s.repo
            .begin_generation(s.discussion, streaming.id, s.gpt, &[], at(2)),
        Err(Error::Invalid(_))
    ));

    let (_, other_root) = s.repo.start_discussion("", s.me, "Other", at(3)).unwrap();
    assert!(matches!(
        s.repo
            .begin_generation(s.discussion, other_root.id, s.gpt, &[], at(4)),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn model_replies_can_be_deleted_by_the_person_but_not_while_running() {
    let mut s = setup();
    let (post, generation) = s
        .repo
        .begin_generation(s.discussion, s.question, s.gpt, &[], at(1))
        .unwrap();
    assert!(matches!(
        s.repo.delete_post(post.id, s.me, UnixMillis(T0)),
        Err(Error::Invalid(_))
    ));
    s.repo
        .finish_generation(generation.id, complete("Unhelpful"), UnixMillis(T0 + 1))
        .unwrap();
    let deleted = s
        .repo
        .delete_post(post.id, s.me, UnixMillis(T0 + 2))
        .unwrap();
    assert!(deleted.deleted_at.is_some());

    // But nobody can edit a model's words into something else.
    let (post, generation) = s
        .repo
        .begin_generation(s.discussion, s.question, s.gpt, &[], at(3))
        .unwrap();
    s.repo
        .finish_generation(generation.id, complete("Original"), UnixMillis(T0 + 4))
        .unwrap();
    assert!(matches!(
        s.repo
            .edit_post(post.id, s.me, "Words put in its mouth", UnixMillis(T0 + 5)),
        Err(Error::Invalid(_))
    ));
}

#[test]
fn changing_the_endpoint_keeps_the_same_provider() {
    let mut repo = SqliteRepository::open_in_memory().unwrap();
    let openai = repo
        .standard_provider(ProviderKind::OpenAi, UnixMillis(0))
        .unwrap();
    repo.set_provider_base_url(openai.id, Some("http://localhost:8080/v1"), UnixMillis(1))
        .unwrap();
    let again = repo
        .standard_provider(ProviderKind::OpenAi, UnixMillis(2))
        .unwrap();
    assert_eq!(again.id, openai.id);
    assert_eq!(again.base_url.as_deref(), Some("http://localhost:8080/v1"));

    assert!(matches!(
        repo.set_provider_base_url(openai.id, Some("ftp://example.com"), UnixMillis(3)),
        Err(Error::Invalid(_))
    ));
    let reset = repo
        .set_provider_base_url(openai.id, Some("  "), UnixMillis(4))
        .unwrap();
    assert_eq!(reset.base_url, None);
}

#[test]
fn aliases_name_exactly_one_model() {
    let mut s = setup();
    let provider = s
        .repo
        .standard_provider(ProviderKind::OpenAi, UnixMillis(0))
        .unwrap()
        .id;
    let mini = s
        .repo
        .set_model(provider, "gpt-5.6-mini", "GPT mini", true)
        .unwrap();

    let gpt = s
        .repo
        .set_model_aliases(
            s.gpt,
            &[
                "@Review".into(),
                "r".into(),
                "review".into(),
                "gpt56".into(),
            ],
        )
        .unwrap();
    assert_eq!(
        gpt.aliases,
        vec!["review", "r"],
        "normalised, deduplicated, own automatic alias dropped"
    );

    // Taken by another model, explicitly or automatically.
    let taken = s
        .repo
        .set_model_aliases(mini.participant.id, &["review".into()]);
    assert!(
        matches!(&taken, Err(Error::Invalid(m)) if m.contains("already names GPT-5.6")),
        "{taken:?}"
    );
    let automatic = s
        .repo
        .set_model_aliases(mini.participant.id, &["gpt56".into()]);
    assert!(matches!(automatic, Err(Error::Invalid(_))));
    assert!(matches!(
        s.repo
            .set_model_aliases(mini.participant.id, &["two words".into()]),
        Err(Error::Invalid(_))
    ));

    // Enabling or renaming a model keeps its aliases.
    let renamed = s.repo.set_model(provider, "gpt-5.6", "GPT", true).unwrap();
    assert_eq!(renamed.aliases, vec!["review", "r"]);
}

#[test]
fn the_default_model_is_remembered_until_unset() {
    let mut s = setup();
    assert_eq!(s.repo.default_model().unwrap(), None);
    s.repo.set_default_model(Some(s.gpt)).unwrap();
    assert_eq!(s.repo.default_model().unwrap(), Some(s.gpt));
    assert!(matches!(
        s.repo.set_default_model(Some(s.me)),
        Err(Error::NotFound(_))
    ));
    s.repo.set_default_model(None).unwrap();
    assert_eq!(s.repo.default_model().unwrap(), None);
}

#[test]
fn every_post_after_the_first_replies_to_something() {
    let mut s = setup();
    let error = s
        .repo
        .add_post(s.discussion, None, s.me, "A new topic", at(2))
        .unwrap_err();
    assert!(
        error.to_string().contains("start a new discussion"),
        "{error}"
    );
    s.repo
        .add_post(s.discussion, Some(s.question), s.me, "A reply", at(3))
        .unwrap();
}
