//! Search speed on a large generated archive: 10,000 discussions and
//! 1,000,000 posts (about 250 MB). Ignored by default because building the
//! archive takes a few minutes. Run it in release mode:
//!
//! ```sh
//! cargo test --release -p nimata-core --test search_performance -- --ignored --nocapture
//! ```
//!
//! The budget, documented in docs/search.md: every query below answers in
//! under 250 ms on a 2021 laptop (Apple M1 Max).

use std::time::{Duration, Instant};

use nimata_core::search::{fold, parse};
use nimata_core::{Repository, SqliteRepository};
use rusqlite::{Connection, functions::FunctionFlags, params};
use uuid::Uuid;

const DISCUSSIONS: usize = 10_000;
const POSTS_PER_DISCUSSION: usize = 100;
const BUDGET: Duration = Duration::from_millis(250);

/// A deterministic generator, so every run searches the same archive.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, below: usize) -> usize {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 33) as usize) % below
    }
}

const VOCABULARY: &[&str] = &[
    "datalog",
    "mapping",
    "engine",
    "oracle",
    "allocation",
    "rules",
    "stratification",
    "negation",
    "override",
    "custody",
    "digital",
    "assets",
    "ledger",
    "threads",
    "context",
    "model",
    "answer",
    "question",
    "review",
    "latency",
    "budget",
    "index",
    "search",
    "local",
    "first",
    "sync",
    "device",
    "privacy",
    "archive",
    "export",
    "import",
    "attachment",
    "graph",
    "νήματα",
    "λόγος",
    "café",
    "naïve",
    "the",
    "a",
    "of",
    "and",
    "to",
    "in",
    "is",
    "that",
    "with",
];

fn sentence(rng: &mut Lcg, words: usize) -> String {
    let mut text: Vec<&str> = (0..words)
        .map(|_| VOCABULARY[rng.next(VOCABULARY.len())])
        .collect();
    // Rare words make selective queries.
    if rng.next(1000) == 0 {
        text.push("zygomorphic");
    }
    text.join(" ")
}

fn build(path: &std::path::Path) -> Uuid {
    let me = {
        let mut repo = SqliteRepository::open(path).unwrap();
        repo.local_user().unwrap().id
    };
    let mut conn = Connection::open(path).unwrap();
    conn.create_scalar_function(
        "nimata_fold",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| Ok(fold(&ctx.get::<String>(0)?)),
    )
    .unwrap();
    let mut rng = Lcg(42);
    let tx = conn.transaction().unwrap();
    {
        let mut discussion = tx
            .prepare("INSERT INTO discussions (id, title, created_at, updated_at, modified_at) VALUES (?1, ?2, ?3, ?3, ?3)")
            .unwrap();
        let mut post = tx
            .prepare(
                "INSERT INTO posts (id, discussion_id, parent_id, author_id, body, created_at, tz_offset_minutes, status, modified_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, 'complete', ?6)",
            )
            .unwrap();
        let mut at: i64 = 1_700_000_000_000;
        for _ in 0..DISCUSSIONS {
            let id = Uuid::now_v7().to_string();
            discussion
                .execute(params![id, sentence(&mut rng, 4), at])
                .unwrap();
            let mut parent: Option<String> = None;
            for _ in 0..POSTS_PER_DISCUSSION {
                at += 60_000;
                let post_id = Uuid::now_v7().to_string();
                let words = 20 + rng.next(120);
                post.execute(params![
                    post_id,
                    id,
                    parent,
                    me.to_string(),
                    sentence(&mut rng, words),
                    at
                ])
                .unwrap();
                parent = Some(post_id);
            }
        }
    }
    tx.commit().unwrap();
    me
}

#[test]
#[ignore = "builds a 1M-post archive; run in release with --ignored"]
fn searching_a_million_posts_stays_within_budget() {
    let dir = tempfile::TempDir::new().unwrap();
    let path = dir.path().join("large.sqlite3");
    let started = Instant::now();
    build(&path);
    println!(
        "built {DISCUSSIONS} discussions, {} posts in {:.1?}",
        DISCUSSIONS * POSTS_PER_DISCUSSION,
        started.elapsed()
    );

    let mut repo = SqliteRepository::open(&path).unwrap();
    let queries = [
        "zygomorphic",
        "stratif",
        "datalog oracle",
        r#""mapping engine""#,
        "νηματα",
        "the",
        "from:me after:2024-01-01 custody",
        r#"discussion:"ledger" privacy"#,
        "from:me on:2024-06-01",
    ];
    let mut slowest = Duration::ZERO;
    for query in queries {
        let parsed = parse(query);
        // Once to warm the cache, then timed.
        repo.search(&parsed, 50).unwrap();
        let started = Instant::now();
        let results = repo.search(&parsed, 50).unwrap();
        let took = started.elapsed();
        slowest = slowest.max(took);
        println!(
            "{took:>10.1?}  {:>3} posts{}  {query}",
            results.posts.len(),
            if results.more_posts { "+" } else { " " }
        );
    }
    assert!(
        slowest <= BUDGET,
        "slowest query took {slowest:?}, over the {BUDGET:?} budget"
    );
}
