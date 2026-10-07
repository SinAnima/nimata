//! Search syntax and results.
//!
//! A query is free text plus optional filters:
//!
//! | Filter               | Meaning                                         |
//! | -------------------- | ----------------------------------------------- |
//! | `from:claude`        | written by a participant whose name or alias starts with "claude"; `from:me` is you |
//! | `after:2026-09-01`   | written on or after that day (your local time)  |
//! | `before:2026-10-01`  | written before that day                         |
//! | `on:2026-09-15`      | written on that day                             |
//! | `discussion:"Digital Assets"` | in discussions whose title contains that text |
//! | `in:archived`        | also search archived discussions               |
//!
//! Words match their beginnings ("stratif" finds "stratification"), except
//! single letters, which match only themselves. Matches are newest first;
//! "quoted text" matches as a phrase. Unknown `word:value` tokens are
//! treated as text, so pasted URLs and times still work.

use chrono::{Local, NaiveDate, TimeZone};
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;
use uuid::Uuid;

use crate::time::UnixMillis;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "text")]
pub enum Term {
    Word(String),
    Phrase(String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQuery {
    pub terms: Vec<Term>,
    /// Lowercased names or aliases from `from:`.
    pub from: Vec<String>,
    pub after: Option<UnixMillis>,
    pub before: Option<UnixMillis>,
    /// Lowercased text from `discussion:`.
    pub discussion: Option<String>,
    pub include_archived: bool,
    /// Parts of the query that could not be used, explained.
    pub warnings: Vec<String>,
}

impl SearchQuery {
    pub fn is_empty(&self) -> bool {
        self.terms.is_empty()
            && self.from.is_empty()
            && self.after.is_none()
            && self.before.is_none()
            && self.discussion.is_none()
    }

    /// The FTS5 query for the text terms, or `None` without any. Each word
    /// of two or more letters matches as a prefix; phrases match exactly. Terms are folded like the
    /// index, and everything is quoted, so user input can never be read as
    /// FTS5 syntax.
    pub fn fts(&self) -> Option<String> {
        let quote = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
        let parts: Vec<String> = self
            .terms
            .iter()
            .filter_map(|term| match term {
                // A single letter would match most words, so it matches
                // only itself.
                Term::Word(w) => match fold(w) {
                    w if w.is_empty() => None,
                    w if w.chars().count() == 1 => Some(quote(&w)),
                    w => Some(format!("{}*", quote(&w))),
                },
                Term::Phrase(p) => {
                    let words = words(&fold(p));
                    (!words.is_empty()).then(|| quote(&words.join(" ")))
                }
            })
            .collect();
        (!parts.is_empty()).then(|| parts.join(" "))
    }

    /// The words and phrases to highlight in a post opened from a result.
    pub fn highlight_terms(&self) -> Vec<String> {
        self.terms
            .iter()
            .map(|t| match t {
                Term::Word(w) | Term::Phrase(w) => w.clone(),
            })
            .collect()
    }
}

/// Splits a query into tokens, keeping "quoted text" and `key:"quoted value"`
/// together.
fn tokens(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in input.chars() {
        match c {
            '"' => {
                current.push(c);
                quoted = !quoted;
            }
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn unquote(s: &str) -> String {
    s.trim_matches('"').to_string()
}

/// Local midnight at the start of `date`, as a UTC instant.
fn start_of_day(date: NaiveDate) -> Option<UnixMillis> {
    let midnight = date.and_hms_opt(0, 0, 0)?;
    let local = Local.from_local_datetime(&midnight).earliest()?;
    Some(UnixMillis(local.timestamp_millis()))
}

fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()
}

/// Splits words on characters FTS5's tokenizer would also split on, so
/// "GPT-5.6" searches as "GPT" "5" "6" rather than failing to match.
fn words(token: &str) -> Vec<String> {
    token
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

pub fn parse(input: &str) -> SearchQuery {
    let mut query = SearchQuery::default();
    for token in tokens(input) {
        let filter = token.split_once(':').and_then(|(key, value)| {
            let key = key.to_ascii_lowercase();
            let known =
                ["from", "after", "before", "on", "discussion", "in"].contains(&key.as_str());
            (known && !value.is_empty()).then(|| (key, unquote(value)))
        });
        let Some((key, value)) = filter else {
            if token.starts_with('"') {
                let phrase = unquote(&token);
                if !phrase.trim().is_empty() {
                    query.terms.push(Term::Phrase(phrase));
                }
            } else {
                query
                    .terms
                    .extend(words(&token).into_iter().map(Term::Word));
            }
            continue;
        };
        match key.as_str() {
            "from" => query.from.push(value.to_lowercase()),
            "discussion" => query.discussion = Some(value.to_lowercase()),
            "in" if value.eq_ignore_ascii_case("archived") => query.include_archived = true,
            "in" => query.warnings.push(format!(
                "in:{value} is not a filter; did you mean in:archived?"
            )),
            "after" | "before" | "on" => match parse_date(&value) {
                None => query.warnings.push(format!(
                    "{key}:{value} is not a date; write it as YYYY-MM-DD"
                )),
                Some(date) => {
                    let next = date.succ_opt().and_then(start_of_day);
                    match key.as_str() {
                        "after" => query.after = start_of_day(date),
                        "before" => query.before = start_of_day(date),
                        _ => {
                            query.after = start_of_day(date);
                            query.before = next;
                        }
                    }
                }
            },
            _ => unreachable!("only known keys reach here"),
        }
    }
    query
}

/// Lowercases `text` and removes accents in any script. The search index
/// holds folded text, and queries are folded the same way.
pub fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .nfd()
        .filter(|c| !is_combining_mark(*c))
        .map(|c| if c == 'ς' { 'σ' } else { c })
        .collect()
}

/// The words of `text` as (start, end) byte ranges and their folded form,
/// split the way the index splits them.
fn spans(text: &str) -> Vec<(usize, usize, String)> {
    let mut spans = Vec::new();
    let mut start = None;
    for (i, c) in text
        .char_indices()
        .chain(std::iter::once((text.len(), ' ')))
    {
        match (c.is_alphanumeric(), start) {
            (true, None) => start = Some(i),
            (false, Some(s)) => {
                spans.push((s, i, fold(&text[s..i])));
                start = None;
            }
            _ => {}
        }
    }
    spans
}

/// Which words of `text` match the query's terms, as indexes into `spans`.
fn matching(spans: &[(usize, usize, String)], query: &SearchQuery) -> Vec<bool> {
    let mut hit = vec![false; spans.len()];
    for term in &query.terms {
        match term {
            Term::Word(w) => {
                let w = fold(w);
                let exact = w.chars().count() == 1;
                for (i, span) in spans.iter().enumerate() {
                    if !w.is_empty() && (span.2 == w || (!exact && span.2.starts_with(&w))) {
                        hit[i] = true;
                    }
                }
            }
            Term::Phrase(p) => {
                let phrase = words(&fold(p));
                if phrase.is_empty() || phrase.len() > spans.len() {
                    continue;
                }
                for start in 0..=spans.len() - phrase.len() {
                    if phrase.iter().zip(&spans[start..]).all(|(w, s)| *w == s.2) {
                        hit[start..start + phrase.len()].fill(true);
                    }
                }
            }
        }
    }
    hit
}

/// Copies `text[from..to]` with matching words marked and whitespace runs
/// collapsed to single spaces.
fn marked(
    text: &str,
    spans: &[(usize, usize, String)],
    hit: &[bool],
    from: usize,
    to: usize,
) -> String {
    let mut out = String::new();
    let mut at = spans.get(from).map_or(0, |s| s.0);
    let mut open = false;
    for i in from..to {
        let (start, end, _) = &spans[i];
        out.push_str(&text[at..*start]);
        if hit[i] && !open {
            out.push(MATCH_START);
            open = true;
        }
        out.push_str(&text[*start..*end]);
        // Keep a phrase's words in one mark.
        if open
            && !(i + 1 < to
                && hit[i + 1]
                && text[*end..spans[i + 1].0].chars().all(char::is_whitespace))
        {
            out.push(MATCH_END);
            open = false;
        }
        at = *end;
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `text` with every match of the query's terms marked.
pub fn mark(text: &str, query: &SearchQuery) -> String {
    let spans = spans(text);
    let hit = matching(&spans, query);
    let mut out = marked(text, &spans, &hit, 0, spans.len());
    // Keep anything after the last word, such as a closing "?".
    if let Some(last) = spans.last() {
        out.push_str(text[last.1..].trim_end());
    } else {
        out = text.trim().to_string();
    }
    out
}

/// A window of about `width` words around the first match (or from the
/// start, without one), matches marked, with "…" where text was cut.
pub fn snippet(text: &str, query: &SearchQuery, width: usize) -> String {
    let spans = spans(text);
    if spans.is_empty() {
        return text.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    let hit = matching(&spans, query);
    let first = hit.iter().position(|h| *h).unwrap_or(0);
    let from = first.saturating_sub(width / 3);
    let to = (from + width).min(spans.len());
    let from = to.saturating_sub(width).min(from);
    let mut out = String::new();
    if from > 0 {
        out.push_str("… ");
    }
    out.push_str(&marked(text, &spans, &hit, from, to));
    if to < spans.len() {
        out.push_str(" …");
    } else {
        out.push_str(text[spans[to - 1].1..].trim_end());
    }
    out
}

/// Marks around matched text in snippets: private-use characters that never
/// appear in ordinary text.
pub const MATCH_START: char = '\u{E000}';
pub const MATCH_END: char = '\u{E001}';

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PostHit {
    pub post_id: Uuid,
    pub discussion_id: Uuid,
    pub discussion_title: String,
    pub author_name: String,
    pub created_at: UnixMillis,
    /// Text around the match, with matches between [`MATCH_START`] and
    /// [`MATCH_END`].
    pub snippet: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscussionHit {
    pub discussion_id: Uuid,
    /// The title, with matches marked like snippets.
    pub title: String,
    pub last_activity_at: UnixMillis,
    pub archived: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    pub posts: Vec<PostHit>,
    /// More posts matched than were returned.
    pub more_posts: bool,
    pub discussions: Vec<DiscussionHit>,
    pub warnings: Vec<String>,
    /// Terms to highlight when a result is opened.
    pub highlight: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words_of(q: &SearchQuery) -> Vec<&str> {
        q.terms
            .iter()
            .map(|t| match t {
                Term::Word(w) | Term::Phrase(w) => w.as_str(),
            })
            .collect()
    }

    #[test]
    fn free_text_and_phrases() {
        let q = parse(r#"Datalog "mapping engine" Oracle"#);
        assert_eq!(
            q.terms,
            vec![
                Term::Word("Datalog".into()),
                Term::Phrase("mapping engine".into()),
                Term::Word("Oracle".into()),
            ]
        );
        assert_eq!(q.fts().unwrap(), r#""datalog"* "mapping engine" "oracle"*"#);
    }

    #[test]
    fn filters_from_the_documented_examples() {
        let q = parse(r#"from:GPT after:2026-09-01 discussion:"Digital Assets" in:archived"#);
        assert!(q.terms.is_empty());
        assert_eq!(q.from, vec!["gpt"]);
        assert!(q.after.is_some());
        assert_eq!(q.discussion.as_deref(), Some("digital assets"));
        assert!(q.include_archived);
        assert!(q.warnings.is_empty());
    }

    #[test]
    fn on_covers_one_local_day() {
        let q = parse("on:2026-09-15");
        let (after, before) = (q.after.unwrap().0, q.before.unwrap().0);
        assert!(before > after);
        assert!(
            (before - after - 86_400_000).abs() <= 3_600_000,
            "a day, give or take a DST hour"
        );
    }

    #[test]
    fn unknown_keys_and_urls_stay_text() {
        let q = parse("see https://example.com at 10:30");
        assert_eq!(
            words_of(&q),
            vec!["see", "https", "example", "com", "at", "10", "30"]
        );
        assert!(q.from.is_empty());
    }

    #[test]
    fn model_names_split_like_the_index_does() {
        assert_eq!(words_of(&parse("GPT-5.6")), vec!["GPT", "5", "6"]);
    }

    #[test]
    fn bad_filters_are_explained_not_fatal() {
        let q = parse("after:yesterday in:trash oracle");
        assert_eq!(words_of(&q), vec!["oracle"]);
        assert_eq!(q.warnings.len(), 2);
        assert!(q.warnings[0].contains("YYYY-MM-DD"));
        assert!(q.warnings[1].contains("in:archived"));
    }

    #[test]
    fn folding_removes_case_and_accents_in_any_script() {
        assert_eq!(fold("Νήματα"), "νηματα");
        assert_eq!(fold("ΆΈΉΊΌΎΏ ϊΰ"), "αεηιουω ιυ");
        assert_eq!(fold("λόγος"), "λογοσ");
        assert_eq!(fold("Café Ångström naïve"), "cafe angstrom naive");
        assert_eq!(fold("Straße"), "straße");
    }

    #[test]
    fn folded_queries_find_accented_text() {
        assert_eq!(parse("ΝΉΜΑΤΑ").fts().unwrap(), r#""νηματα"*"#);
        assert_eq!(
            parse(r#""Mapping  Engine""#).fts().unwrap(),
            r#""mapping engine""#
        );
    }

    #[test]
    fn snippets_mark_matches_in_the_original_text() {
        let q = parse(r#"νηματα "the oracle""#);
        let text = "Νήματα means threads;\n\nthe Oracle angle is separate.";
        assert_eq!(
            mark(text, &q),
            "\u{E000}Νήματα\u{E001} means threads; \u{E000}the Oracle\u{E001} angle is separate."
        );
        let long = format!("{} needle {}", "word ".repeat(50), "tail ".repeat(50));
        let s = snippet(&long, &parse("need"), 12);
        assert!(s.starts_with("… word"), "{s}");
        assert!(s.contains("\u{E000}needle\u{E001}"));
        assert!(s.ends_with(" …"));
        assert_eq!(snippet("Short one?", &parse(""), 12), "Short one?");
    }

    #[test]
    fn an_empty_query_is_empty() {
        assert!(parse("   ").is_empty());
        assert_eq!(parse("").fts(), None);
        assert_eq!(parse("a").fts().unwrap(), r#""a""#);
        assert!(!parse("from:me").is_empty());
    }
}
