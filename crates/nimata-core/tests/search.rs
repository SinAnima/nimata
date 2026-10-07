//! Search over a small corpus with people, models, archives, and deletions.

use nimata_core::domain::{GenerationStatus, ProviderKind, ProviderMetadata};
use nimata_core::repository::GenerationOutcome;
use nimata_core::search::{MATCH_END, MATCH_START, parse};
use nimata_core::{Repository, SqliteRepository, Timestamp, UnixMillis};
use uuid::Uuid;

/// 2026-09-20 12:00 UTC.
const T0: i64 = 1_789_905_600_000;
const DAY: i64 = 86_400_000;

fn at(days: i64) -> Timestamp {
    Timestamp {
        at: UnixMillis(T0 + days * DAY),
        offset_minutes: 0,
    }
}

struct Corpus {
    repo: SqliteRepository,
    me: Uuid,
    claude: Uuid,
    datalog: Uuid,
    stratification: Uuid,
}

/// Writes a model reply directly through the generation API.
fn model_reply(
    repo: &mut SqliteRepository,
    discussion: Uuid,
    parent: Uuid,
    model: Uuid,
    body: &str,
    day: i64,
) -> Uuid {
    let (post, generation) = repo
        .begin_generation(discussion, parent, model, &[parent], at(day))
        .unwrap();
    repo.finish_generation(
        generation.id,
        GenerationOutcome {
            status: GenerationStatus::Complete,
            body: body.into(),
            metadata: Some(ProviderMetadata {
                provider: "anthropic".into(),
                model: None,
                response_id: None,
                request_id: None,
                input_tokens: None,
                output_tokens: None,
                incomplete_reason: None,
            }),
            error: None,
        },
        at(day).at,
    )
    .unwrap();
    post.id
}

fn corpus() -> Corpus {
    let mut repo = SqliteRepository::open_in_memory().unwrap();
    let me = repo.local_user().unwrap().id;
    repo.rename_participant(me, "Thanos").unwrap();
    let anthropic = repo
        .standard_provider(ProviderKind::Anthropic, UnixMillis(0))
        .unwrap();
    let claude = repo
        .set_model(anthropic.id, "claude-opus-5-5", "Claude Opus", true)
        .unwrap()
        .participant
        .id;
    repo.set_model_aliases(claude, &["critic".into()]).unwrap();

    let (d1, q1) = repo
        .start_discussion(
            "Datalog for the mapping engine",
            me,
            "Could Datalog replace the Oracle allocation rules?",
            at(0),
        )
        .unwrap();
    let stratification = model_reply(
        &mut repo,
        d1.id,
        q1.id,
        claude,
        "Only with stratification: overrides behave like negation.",
        2,
    );

    let (d2, q2) = repo
        .start_discussion(
            "Digital Assets",
            me,
            "Custody rules for digital assets.",
            at(10),
        )
        .unwrap();
    repo.add_post(
        d2.id,
        Some(q2.id),
        me,
        "Νήματα means threads; the Oracle angle is separate.",
        at(11),
    )
    .unwrap();

    let (d3, _) = repo
        .start_discussion("Old Oracle notes", me, "Archived Oracle musings.", at(20))
        .unwrap();
    repo.set_archived(d3.id, true, UnixMillis(T0 + 21 * DAY))
        .unwrap();

    let (d4, q4) = repo
        .start_discussion("Gone", me, "Deleted Oracle discussion.", at(22))
        .unwrap();
    repo.delete_discussion(d4.id, UnixMillis(T0 + 23 * DAY))
        .unwrap();
    let regret = repo
        .add_post(d1.id, Some(q1.id), me, "Regrettable Oracle remark.", at(24))
        .unwrap();
    repo.delete_post(regret.id, me, UnixMillis(T0 + 25 * DAY))
        .unwrap();
    let _ = q4;

    Corpus {
        repo,
        me,
        claude,
        datalog: d1.id,
        stratification,
    }
}

fn bodies(results: &nimata_core::search::SearchResults) -> Vec<String> {
    results
        .posts
        .iter()
        .map(|p| p.snippet.replace([MATCH_START, MATCH_END], ""))
        .collect()
}

#[test]
fn words_match_their_beginnings_and_deleted_posts_never_match() {
    let mut c = corpus();
    let r = c.repo.search(&parse("oracle"), 50).unwrap();
    let found = bodies(&r);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found.iter().all(|b| !b.contains("Regrettable")
            && !b.contains("Deleted")
            && !b.contains("Archived"))
    );

    let r = c.repo.search(&parse("stratif"), 50).unwrap();
    assert_eq!(r.posts[0].post_id, c.stratification);
    assert_eq!(r.posts[0].author_name, "Claude Opus");
    assert_eq!(
        r.posts[0].discussion_title,
        "Datalog for the mapping engine"
    );
    assert!(
        r.posts[0]
            .snippet
            .contains(&format!("{MATCH_START}stratification{MATCH_END}"))
    );
}

#[test]
fn phrases_match_exactly() {
    let mut c = corpus();
    assert_eq!(
        c.repo
            .search(&parse(r#""allocation rules""#), 50)
            .unwrap()
            .posts
            .len(),
        1
    );
    assert_eq!(
        c.repo
            .search(&parse(r#""rules allocation""#), 50)
            .unwrap()
            .posts
            .len(),
        0
    );
}

#[test]
fn accents_do_not_matter() {
    let mut c = corpus();
    assert_eq!(c.repo.search(&parse("νηματα"), 50).unwrap().posts.len(), 1);
    assert_eq!(c.repo.search(&parse("ΝΉΜΑΤΑ"), 50).unwrap().posts.len(), 1);
}

#[test]
fn from_finds_people_and_models_by_name_alias_or_me() {
    let mut c = corpus();
    for query in ["from:claude", "from:cl", "from:critic", "from:claudeopus"] {
        let r = c.repo.search(&parse(query), 50).unwrap();
        assert_eq!(r.posts.len(), 1, "{query}");
        assert_eq!(r.posts[0].post_id, c.stratification);
    }
    let mine = c.repo.search(&parse("from:me oracle"), 50).unwrap();
    assert_eq!(mine.posts.len(), 2);
    assert_eq!(mine.posts[0].author_name, "Thanos");
    assert_eq!(
        c.repo
            .search(&parse("from:thanos oracle"), 50)
            .unwrap()
            .posts
            .len(),
        2
    );

    let nobody = c.repo.search(&parse("from:grok"), 50).unwrap();
    assert!(nobody.posts.is_empty());
    assert!(nobody.warnings[0].contains("No one is called"));
    let _ = (c.me, c.claude);
}

#[test]
fn dates_discussions_and_archives_narrow_results() {
    let mut c = corpus();
    let day = |offset: i64| {
        chrono::DateTime::from_timestamp_millis(T0 + offset * DAY)
            .unwrap()
            .format("%Y-%m-%d")
            .to_string()
    };
    assert_eq!(
        c.repo
            .search(&parse(&format!("oracle after:{}", day(5))), 50)
            .unwrap()
            .posts
            .len(),
        1
    );
    assert_eq!(
        c.repo
            .search(&parse(&format!("oracle before:{}", day(5))), 50)
            .unwrap()
            .posts
            .len(),
        1
    );
    assert_eq!(
        c.repo
            .search(&parse(&format!("on:{}", day(2))), 50)
            .unwrap()
            .posts
            .len(),
        1
    );

    let r = c
        .repo
        .search(&parse(r#"discussion:"digital assets""#), 50)
        .unwrap();
    assert_eq!(r.posts.len(), 2);
    assert!(
        r.posts
            .iter()
            .all(|p| p.discussion_title == "Digital Assets")
    );

    assert_eq!(c.repo.search(&parse("musings"), 50).unwrap().posts.len(), 0);
    let archived = c.repo.search(&parse("musings in:archived"), 50).unwrap();
    assert_eq!(archived.posts.len(), 1);
}

#[test]
fn titles_are_searched_too() {
    let mut c = corpus();
    let r = c.repo.search(&parse("mapping"), 50).unwrap();
    assert_eq!(r.discussions.len(), 1);
    assert_eq!(r.discussions[0].discussion_id, c.datalog);
    assert_eq!(
        r.discussions[0].title,
        format!("Datalog for the {MATCH_START}mapping{MATCH_END} engine")
    );
    assert!(
        c.repo
            .search(&parse("gone"), 50)
            .unwrap()
            .discussions
            .is_empty(),
        "deleted titles never match"
    );
}

#[test]
fn edits_and_streamed_text_are_found_under_their_current_wording() {
    let mut c = corpus();
    let r = c.repo.search(&parse("custody"), 50).unwrap();
    let post = r.posts[0].post_id;
    c.repo
        .edit_post(
            post,
            c.me,
            "Safekeeping rules for digital assets.",
            UnixMillis(T0 + 30 * DAY),
        )
        .unwrap();
    assert!(
        c.repo
            .search(&parse("custody"), 50)
            .unwrap()
            .posts
            .is_empty()
    );
    assert_eq!(
        c.repo
            .search(&parse("safekeeping"), 50)
            .unwrap()
            .posts
            .len(),
        1
    );

    let root = post_root(&mut c);
    let (streaming, generation) = c
        .repo
        .begin_generation(c.datalog, root, c.claude, &[], at(31))
        .unwrap();
    c.repo
        .update_generation(
            generation.id,
            GenerationStatus::Streaming,
            "Partial thoughts on materialisation",
        )
        .unwrap();
    let r = c.repo.search(&parse("materialisation"), 50).unwrap();
    assert_eq!(r.posts[0].post_id, streaming.id);
}

fn post_root(c: &mut Corpus) -> Uuid {
    c.repo.get_discussion(c.datalog).unwrap().posts[0].id
}

#[test]
fn hostile_input_cannot_break_or_bend_a_query() {
    let mut c = corpus();
    for input in [
        r#"NEAR(oracle rules)"#,
        r#"oracle OR custody"#,
        r#"" " "#,
        r#"body:oracle"#,
        r#"*"#,
        r#"oracle" OR "custody"#,
        r#"'; DROP TABLE posts; --"#,
        r#"discussion:%"#,
        r#"discussion:_"#,
    ] {
        let r = c.repo.search(&parse(input), 50);
        assert!(r.is_ok(), "{input}: {r:?}");
    }
    // OR is a word, not an operator: nothing contains both "oracle" and "or".
    assert!(
        c.repo
            .search(&parse("oracle OR custody"), 50)
            .unwrap()
            .posts
            .is_empty()
    );
    // % and _ in discussion: are literal.
    assert!(
        c.repo
            .search(&parse("discussion:%"), 50)
            .unwrap()
            .posts
            .is_empty()
    );
}

#[test]
fn results_are_limited_with_a_marker() {
    let mut c = corpus();
    let r = c.repo.search(&parse("oracle"), 1).unwrap();
    assert_eq!(r.posts.len(), 1);
    assert!(r.more_posts);
}

#[test]
fn recent_searches_keep_the_last_ten_most_recent_first() {
    let mut c = corpus();
    for n in 0..12 {
        c.repo
            .record_search(&format!("query {n}"), UnixMillis(n))
            .unwrap();
    }
    c.repo.record_search("query 3", UnixMillis(100)).unwrap();
    c.repo.record_search("   ", UnixMillis(101)).unwrap();
    let recent = c.repo.recent_searches(20).unwrap();
    assert_eq!(recent.len(), 10);
    assert_eq!(recent[0], "query 3");
    assert_eq!(recent[1], "query 11");
    c.repo.clear_recent_searches().unwrap();
    assert!(c.repo.recent_searches(20).unwrap().is_empty());
}

#[test]
fn a_restored_backup_is_searchable() {
    let dir = tempfile::TempDir::new().unwrap();
    let c = corpus();
    let backup = dir.path().join("backup.sqlite3");
    c.repo.backup_to(&backup).unwrap();
    let mut fresh = SqliteRepository::open(&dir.path().join("fresh.sqlite3")).unwrap();
    fresh.restore_from(&backup).unwrap();
    assert_eq!(
        fresh
            .search(&parse("stratification"), 50)
            .unwrap()
            .posts
            .len(),
        1
    );
}
