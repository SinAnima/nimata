//! Static fixture discussions used until persistence exists (Stage 0).
//!
//! Times are relative to `now` so the sidebar groups look realistic whenever
//! the app is launched.

use uuid::Uuid;

use crate::domain::{Discussion, DiscussionView, Participant, ParticipantKind, Post, PostStatus};
use crate::time::UnixMillis;

pub struct Fixtures {
    pub participants: Vec<Participant>,
    pub views: Vec<DiscussionView>,
}

struct Cast {
    me: Participant,
    gpt: Participant,
    claude: Participant,
    grok: Participant,
}

fn participant(
    kind: ParticipantKind,
    name: &str,
    provider: Option<&str>,
    model: Option<&str>,
) -> Participant {
    Participant {
        id: Uuid::now_v7(),
        kind,
        display_name: name.to_string(),
        provider: provider.map(str::to_string),
        model: model.map(str::to_string),
    }
}

/// Builds the posts of one discussion in the order they were written.
struct Thread<'a> {
    now: UnixMillis,
    discussion_id: Uuid,
    posts: Vec<Post>,
    cast: &'a Cast,
}

impl<'a> Thread<'a> {
    fn new(now: UnixMillis, cast: &'a Cast) -> Self {
        Self {
            now,
            discussion_id: Uuid::now_v7(),
            posts: Vec::new(),
            cast,
        }
    }

    fn post(
        &mut self,
        parent: Option<Uuid>,
        author: &Participant,
        minutes_ago: i64,
        offset: i32,
        body: &str,
    ) -> Uuid {
        let id = Uuid::now_v7();
        self.posts.push(Post {
            id,
            discussion_id: self.discussion_id,
            parent_id: parent,
            author_id: author.id,
            body: body.trim().to_string(),
            created_at: self.now.minus_minutes(minutes_ago),
            tz_offset_minutes: offset,
            edited_at: None,
            status: PostStatus::Complete,
        });
        id
    }

    fn finish(self, title: &str) -> DiscussionView {
        let created_at = self
            .posts
            .iter()
            .map(|p| p.created_at)
            .min()
            .unwrap_or(self.now);
        let updated_at = self
            .posts
            .iter()
            .map(|p| p.created_at)
            .max()
            .unwrap_or(self.now);
        let author_ids: Vec<Uuid> = self.posts.iter().map(|p| p.author_id).collect();
        let participants = [
            &self.cast.me,
            &self.cast.gpt,
            &self.cast.claude,
            &self.cast.grok,
        ]
        .into_iter()
        .filter(|p| author_ids.contains(&p.id))
        .cloned()
        .collect();
        DiscussionView {
            discussion: Discussion {
                id: self.discussion_id,
                title: title.to_string(),
                created_at,
                updated_at,
                archived_at: None,
                pinned_at: None,
            },
            posts: self.posts,
            participants,
        }
    }
}

const NEW_YORK: i32 = -240;
const ATHENS: i32 = 180;

pub fn build(now: UnixMillis) -> Fixtures {
    let cast = Cast {
        me: participant(ParticipantKind::Human, "Thanos", None, None),
        gpt: participant(
            ParticipantKind::Model,
            "GPT-5.6",
            Some("openai"),
            Some("gpt-5.6"),
        ),
        claude: participant(
            ParticipantKind::Model,
            "Claude Sonnet",
            Some("anthropic"),
            Some("claude-sonnet-5-5"),
        ),
        grok: participant(ParticipantKind::Model, "Grok", Some("xai"), Some("grok-5")),
    };

    let views = vec![
        datalog(now, &cast),
        couchdb(now, &cast),
        local_first(now, &cast),
    ];
    let participants = vec![cast.me, cast.gpt, cast.claude, cast.grok];
    Fixtures {
        participants,
        views,
    }
}

fn datalog(now: UnixMillis, cast: &Cast) -> DiscussionView {
    let mut t = Thread::new(now, cast);
    let q = t.post(None, &cast.me, 95, NEW_YORK, "
Could Datalog replace our mapping engine? The current engine is a pile of hand-ordered rules, and every new source format means another special case.
");
    let gpt = t.post(Some(q), &cast.gpt, 94, NEW_YORK, "
Yes, but there are three separate issues hiding in that question.

First, expressiveness: most mapping rules are joins plus a little arithmetic, which Datalog handles well. Second, evaluation order: semi-naive evaluation removes the need to hand-order rules. Third, and hardest, is the escape hatch for the few transformations that genuinely need procedural code.
");
    let grok = t.post(Some(gpt), &cast.grok, 92, NEW_YORK, "
I disagree with the second assumption. Removing hand ordering only helps if the rules are actually monotonic. Mapping engines usually accumulate defaults and overrides, which is negation in disguise. You would need stratification, and the strata end up looking a lot like the ordering you wanted to remove.
");
    let mine = t.post(Some(gpt), &cast.me, 90, NEW_YORK, "
The Oracle implementation actually solved the override problem with a priority column rather than negation. Every rule emits a candidate with a priority, and a final aggregation picks the winner.
");
    t.post(Some(grok), &cast.claude, 88, NEW_YORK, "
Grok's point holds for naive translations, but priority-based candidates sidestep it: emitting candidates is monotonic, and only the final max-by-priority step is an aggregate. That is a single stratum boundary, not a reconstruction of the old ordering.
");
    t.post(Some(mine), &cast.claude, 86, NEW_YORK, "
That changes the answer substantially. If overrides are already expressed as prioritized candidates, the migration is mostly mechanical: each rule becomes a Datalog rule producing (target, value, priority), plus one aggregation per target field.

The remaining risk is performance on wide records, which is worth measuring before committing.
");
    t.finish("Datalog for the mapping engine")
}

fn couchdb(now: UnixMillis, cast: &Cast) -> DiscussionView {
    let mut t = Thread::new(now, cast);
    let q = t.post(
        None,
        &cast.me,
        60 * 26,
        ATHENS,
        "Should Nimata use CouchDB?",
    );
    let attraction = t.post(
        Some(q),
        &cast.me,
        60 * 26 - 2,
        ATHENS,
        "The attraction is replication.",
    );
    let mobile = t.post(
        Some(attraction),
        &cast.me,
        60 * 26 - 5,
        ATHENS,
        "But mobile changes the constraints.",
    );
    let claude = t.post(Some(mobile), &cast.claude, 60 * 25, ATHENS, "
Mobile is exactly where the CouchDB model gets awkward. You do not want a database server on the phone, so you end up with a different local store that speaks the replication protocol, and the protocol becomes the real dependency rather than CouchDB itself.
");
    t.post(
        Some(claude),
        &cast.me,
        60 * 24,
        ATHENS,
        "
So the question is really: which replication protocol, and can it stay an adapter?
",
    );
    t.finish("Should Nimata use CouchDB?")
}

fn local_first(now: UnixMillis, cast: &Cast) -> DiscussionView {
    let mut t = Thread::new(now, cast);
    let q = t.post(None, &cast.me, 60 * 24 * 4, NEW_YORK, "
Collecting reading on local-first software. What should be on the list beyond the Ink & Switch essay?
");
    let gpt = t.post(Some(q), &cast.gpt, 60 * 24 * 4 - 3, NEW_YORK, "
Three additions: the original CRDT papers for the theory, the Automerge and Yjs documentation for practical trade-offs, and any write-up on how Apple Notes handles sync, since it is a widely used local-first system in disguise.
");
    t.post(Some(gpt), &cast.grok, 60 * 24 * 4 - 10, NEW_YORK, "
Add something on schema evolution. Most local-first write-ups skip it, and it is where real applications suffer once data lives on many devices at different versions.
");
    t.finish("Reading list: local-first software")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn fixtures() -> Fixtures {
        build(UnixMillis(1_790_000_000_000))
    }

    #[test]
    fn has_three_discussions_with_human_and_model_authors() {
        let f = fixtures();
        assert_eq!(f.views.len(), 3);
        let kinds: HashSet<_> = f.participants.iter().map(|p| p.kind).collect();
        assert!(kinds.contains(&ParticipantKind::Human));
        assert!(kinds.contains(&ParticipantKind::Model));
    }

    #[test]
    fn every_parent_is_an_earlier_post_in_the_same_discussion() {
        for view in fixtures().views {
            for post in &view.posts {
                assert_eq!(post.discussion_id, view.discussion.id);
                if let Some(parent_id) = post.parent_id {
                    let parent = view
                        .posts
                        .iter()
                        .find(|p| p.id == parent_id)
                        .expect("parent exists");
                    assert!(parent.created_at < post.created_at, "parent precedes reply");
                }
            }
        }
    }

    #[test]
    fn every_author_is_listed_as_a_participant() {
        for view in fixtures().views {
            for post in &view.posts {
                assert!(view.participants.iter().any(|p| p.id == post.author_id));
            }
        }
    }

    #[test]
    fn fixtures_include_a_reply_that_is_not_to_the_previous_post() {
        // Proves the chronological stream differs from the reply tree.
        let view = &fixtures().views[0];
        let out_of_line = view
            .posts
            .windows(2)
            .any(|w| w[1].parent_id.is_some_and(|parent| parent != w[0].id));
        assert!(out_of_line);
    }
}
