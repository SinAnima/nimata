//! What a model is shown when asked to reply.
//!
//! By default: the reply chain from the first post down to the post being
//! answered. In addition, posts that anyone in that chain chose as context
//! ("Include as context") are carried down to every later reply, and sent in
//! a labelled message so the model can tell them apart from the chain.
//! Other discussions are never included.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::{Participant, Post, PostStatus};
use crate::error::{Error, Result};
use crate::providers::{Message, ModelRequest, Role};

/// The posts from the root of the thread down to `target`, oldest first.
pub fn ancestry(posts: &[Post], target: Uuid) -> Result<Vec<&Post>> {
    let by_id: HashMap<Uuid, &Post> = posts.iter().map(|p| (p.id, p)).collect();
    let mut chain = Vec::new();
    let mut next = Some(target);
    while let Some(id) = next {
        let post = by_id.get(&id).ok_or(Error::NotFound("post"))?;
        if chain.len() > posts.len() {
            return Err(Error::Invalid(
                "the reply chain loops back on itself".into(),
            ));
        }
        chain.push(*post);
        next = post.parent_id;
    }
    chain.reverse();
    Ok(chain)
}

pub fn instructions(responder: &str) -> String {
    format!(
        "You are {responder}, one participant in a threaded discussion kept in Nimata, a \
         discussion client. The other participants may be people or other AI models. You are \
         shown the thread from its first post down to the post you are replying to; each \
         message written by someone else starts with their name. Posts from other branches of \
         the discussion may be included as context; they are labelled as such and are not what \
         you are replying to. Write your reply to the last message. Do not start your reply \
         with your own name."
    )
}

/// The label that introduces context from other branches.
pub const CONTEXT_LABEL: &str =
    "For context, from another branch of this discussion (not part of the reply chain):";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    /// Part of the reply chain.
    Thread,
    /// Chosen as context with "Include as context".
    Context,
}

/// One message as sent, with the posts it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SentMessage {
    pub role: Role,
    pub text: String,
    pub source: Source,
    pub post_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OmitReason {
    Deleted,
    /// A model reply that failed, was stopped, or is still being written.
    Unfinished,
    /// Left out so the request fits the model's budget.
    Trimmed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Omitted {
    pub post_id: Uuid,
    pub source: Source,
    pub reason: OmitReason,
}

/// Exactly what a model is (or was) sent, and what was left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SentContext {
    pub model: String,
    pub instructions: String,
    pub messages: Vec<SentMessage>,
    pub omitted: Vec<Omitted>,
    /// Rough size: about four characters per token. Providers count
    /// differently; this is only for orientation and trimming.
    pub estimated_tokens: u64,
    pub budget_tokens: u64,
}

impl SentContext {
    /// The request for the provider adapter.
    pub fn request(&self) -> ModelRequest {
        ModelRequest {
            model: self.model.clone(),
            instructions: self.instructions.clone(),
            messages: self
                .messages
                .iter()
                .map(|m| Message {
                    role: m.role,
                    text: m.text.clone(),
                })
                .collect(),
        }
    }

    /// Every post whose text was sent, in order.
    pub fn post_ids(&self) -> Vec<Uuid> {
        self.messages
            .iter()
            .flat_map(|m| m.post_ids.iter().copied())
            .collect()
    }
}

/// A rough token count: about four characters per token, plus a little per
/// message for formatting.
pub fn estimate_tokens(text: &str) -> u64 {
    (text.chars().count() as u64).div_ceil(4) + 4
}

enum Entry<'a> {
    Thread(&'a Post),
    /// Context posts introduced by one post of the chain, sent together.
    Context(Vec<&'a Post>),
}

fn usable(post: &Post) -> Option<OmitReason> {
    if post.deleted_at.is_some() {
        Some(OmitReason::Deleted)
    } else if post.status != PostStatus::Complete {
        Some(OmitReason::Unfinished)
    } else {
        None
    }
}

/// Builds what `responder` is sent when asked to reply to `target`, keeping
/// the estimate within `budget_tokens` where possible.
///
/// Trimming drops the oldest posts in the middle of the chain first, never
/// the first post or the post being replied to, then context posts, oldest
/// first. Everything left out is listed with its reason.
pub fn build(
    posts: &[Post],
    participants: &[Participant],
    target: Uuid,
    responder: &Participant,
    model: &str,
    budget_tokens: u64,
) -> Result<SentContext> {
    let chain = ancestry(posts, target)?;
    let last = chain.last().expect("ancestry includes the target");
    if usable(last).is_some() {
        return Err(Error::Invalid(
            "a model can only reply to a finished post that has not been deleted".into(),
        ));
    }
    let by_id: HashMap<Uuid, &Post> = posts.iter().map(|p| (p.id, p)).collect();
    let in_chain: HashSet<Uuid> = chain.iter().map(|p| p.id).collect();
    let names: HashMap<Uuid, &str> = participants
        .iter()
        .map(|p| (p.id, p.display_name.as_str()))
        .collect();
    let name = |post: &Post| names.get(&post.author_id).copied().unwrap_or("Someone");

    let mut entries: Vec<Entry> = Vec::new();
    let mut omitted: Vec<Omitted> = Vec::new();
    let mut referenced: HashSet<Uuid> = HashSet::new();
    for post in &chain {
        let mut context = Vec::new();
        for id in &post.context_ids {
            if in_chain.contains(id) || !referenced.insert(*id) {
                continue;
            }
            let Some(other) = by_id.get(id) else { continue };
            match usable(other) {
                Some(reason) => omitted.push(Omitted {
                    post_id: *id,
                    source: Source::Context,
                    reason,
                }),
                None => context.push(*other),
            }
        }
        if !context.is_empty() {
            entries.push(Entry::Context(context));
        }
        match usable(post) {
            Some(reason) => omitted.push(Omitted {
                post_id: post.id,
                source: Source::Thread,
                reason,
            }),
            None => entries.push(Entry::Thread(post)),
        }
    }

    let render = |entry: &Entry| -> SentMessage {
        match entry {
            Entry::Thread(post) if post.author_id == responder.id => SentMessage {
                role: Role::Assistant,
                text: post.body.clone(),
                source: Source::Thread,
                post_ids: vec![post.id],
            },
            Entry::Thread(post) => SentMessage {
                role: Role::User,
                text: format!("{}:\n{}", name(post), post.body),
                source: Source::Thread,
                post_ids: vec![post.id],
            },
            Entry::Context(posts) => {
                let parts: Vec<String> = posts
                    .iter()
                    .map(|p| {
                        let who = if p.author_id == responder.id {
                            "You".to_string()
                        } else {
                            name(p).to_string()
                        };
                        format!("{who} wrote:\n{}", p.body)
                    })
                    .collect();
                SentMessage {
                    role: Role::User,
                    text: format!("{CONTEXT_LABEL}\n\n{}", parts.join("\n\n")),
                    source: Source::Context,
                    post_ids: posts.iter().map(|p| p.id).collect(),
                }
            }
        }
    };

    let instructions = instructions(&responder.display_name);
    let total = |entries: &[Entry]| -> u64 {
        estimate_tokens(&instructions)
            + entries
                .iter()
                .map(|e| estimate_tokens(&render(e).text))
                .sum::<u64>()
    };

    // Trim: the oldest middle chain posts first, then context, oldest first.
    while total(&entries) > budget_tokens {
        let thread_positions: Vec<usize> = entries
            .iter()
            .enumerate()
            .filter(|(_, e)| matches!(e, Entry::Thread(_)))
            .map(|(i, _)| i)
            .collect();
        let middle = thread_positions
            .iter()
            .copied()
            .find(|&i| i != thread_positions[0] && i != *thread_positions.last().unwrap());
        let victim = middle.or_else(|| entries.iter().position(|e| matches!(e, Entry::Context(_))));
        let Some(index) = victim else { break };
        match entries.remove(index) {
            Entry::Thread(post) => omitted.push(Omitted {
                post_id: post.id,
                source: Source::Thread,
                reason: OmitReason::Trimmed,
            }),
            Entry::Context(posts) => omitted.extend(posts.iter().map(|p| Omitted {
                post_id: p.id,
                source: Source::Context,
                reason: OmitReason::Trimmed,
            })),
        }
    }

    let messages: Vec<SentMessage> = entries.iter().map(render).collect();
    let estimated_tokens = total(&entries);
    Ok(SentContext {
        model: model.to_string(),
        instructions,
        messages,
        omitted,
        estimated_tokens,
        budget_tokens,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ParticipantKind;
    use crate::time::UnixMillis;

    fn participant(name: &str, kind: ParticipantKind) -> Participant {
        Participant {
            id: Uuid::now_v7(),
            kind,
            display_name: name.into(),
            provider: None,
            model: None,
        }
    }

    fn post(author: &Participant, parent: Option<&Post>, body: &str, at: i64) -> Post {
        Post {
            id: Uuid::now_v7(),
            discussion_id: Uuid::nil(),
            parent_id: parent.map(|p| p.id),
            author_id: author.id,
            body: body.into(),
            created_at: UnixMillis(at),
            tz_offset_minutes: 0,
            edited_at: None,
            deleted_at: None,
            status: PostStatus::Complete,
            provider_metadata: None,
            context_ids: vec![],
        }
    }

    const ROOMY: u64 = 1_000_000;

    #[test]
    fn only_the_thread_down_to_the_target_is_sent_by_default() {
        let me = participant("Thanos", ParticipantKind::Human);
        let gpt = participant("GPT-5.6", ParticipantKind::Model);
        let question = post(&me, None, "Could Datalog replace our mapping engine?", 1);
        let answer = post(&gpt, Some(&question), "Yes, with caveats.", 2);
        let other_branch = post(&me, Some(&question), "Unrelated aside.", 3);
        let follow_up = post(&me, Some(&answer), "What about overrides?", 4);
        let posts = vec![
            question.clone(),
            answer.clone(),
            other_branch,
            follow_up.clone(),
        ];

        let sent = build(
            &posts,
            &[me.clone(), gpt.clone()],
            follow_up.id,
            &gpt,
            "gpt-5.6",
            ROOMY,
        )
        .unwrap();

        assert_eq!(sent.post_ids(), vec![question.id, answer.id, follow_up.id]);
        let request = sent.request();
        assert_eq!(
            request.messages,
            vec![
                Message {
                    role: Role::User,
                    text: "Thanos:\nCould Datalog replace our mapping engine?".into()
                },
                Message {
                    role: Role::Assistant,
                    text: "Yes, with caveats.".into()
                },
                Message {
                    role: Role::User,
                    text: "Thanos:\nWhat about overrides?".into()
                },
            ]
        );
        assert!(sent.omitted.is_empty());
        assert!(request.instructions.contains("You are GPT-5.6"));
    }

    /// GPT and Claude answer the question on sibling branches; I reply to
    /// GPT, including Claude's answer as context; Grok is asked to respond.
    #[test]
    fn a_context_reference_is_sent_labelled_before_the_post_that_chose_it() {
        let me = participant("Thanos", ParticipantKind::Human);
        let gpt = participant("GPT-5.6", ParticipantKind::Model);
        let claude = participant("Claude", ParticipantKind::Model);
        let grok = participant("Grok", ParticipantKind::Model);
        let question = post(&me, None, "Should Nimata use CouchDB?", 1);
        let gpt_answer = post(&gpt, Some(&question), "Yes: replication.", 2);
        let claude_answer = post(&claude, Some(&question), "No: mobile.", 3);
        let mut mine = post(&me, Some(&gpt_answer), "Weigh Claude's point too.", 4);
        mine.context_ids = vec![claude_answer.id];
        let posts = vec![
            question.clone(),
            gpt_answer.clone(),
            claude_answer.clone(),
            mine.clone(),
        ];
        let everyone = [me, gpt, claude, grok.clone()];

        let sent = build(&posts, &everyone, mine.id, &grok, "grok-5", ROOMY).unwrap();

        let sources: Vec<_> = sent.messages.iter().map(|m| m.source).collect();
        assert_eq!(
            sources,
            vec![
                Source::Thread,
                Source::Thread,
                Source::Context,
                Source::Thread
            ]
        );
        assert_eq!(
            sent.messages[2].text,
            format!("{CONTEXT_LABEL}\n\nClaude wrote:\nNo: mobile.")
        );
        assert_eq!(sent.messages[2].post_ids, vec![claude_answer.id]);
        assert_eq!(sent.messages[3].text, "Thanos:\nWeigh Claude's point too.");
    }

    #[test]
    fn references_carry_down_to_later_replies_in_the_chain() {
        let me = participant("Me", ParticipantKind::Human);
        let gpt = participant("GPT", ParticipantKind::Model);
        let root = post(&me, None, "root", 1);
        let aside = post(&me, Some(&root), "an aside", 2);
        let mut referencing = post(&me, Some(&root), "see the aside", 3);
        referencing.context_ids = vec![aside.id];
        let answer = post(&gpt, Some(&referencing), "noted", 4);
        let later = post(&me, Some(&answer), "and now?", 5);
        let posts = vec![root, aside.clone(), referencing, answer, later.clone()];

        let sent = build(&posts, &[me, gpt.clone()], later.id, &gpt, "m", ROOMY).unwrap();
        assert!(
            sent.post_ids().contains(&aside.id),
            "the reference is still there further down"
        );
        assert_eq!(
            sent.post_ids().iter().filter(|id| **id == aside.id).count(),
            1
        );
    }

    #[test]
    fn a_reference_to_the_models_own_post_says_you() {
        let me = participant("Me", ParticipantKind::Human);
        let gpt = participant("GPT", ParticipantKind::Model);
        let root = post(&me, None, "root", 1);
        let earlier = post(&gpt, Some(&root), "my earlier idea", 2);
        let mut mine = post(&me, Some(&root), "revisit it", 3);
        mine.context_ids = vec![earlier.id];
        let posts = vec![root, earlier, mine.clone()];
        let sent = build(&posts, &[me, gpt.clone()], mine.id, &gpt, "m", ROOMY).unwrap();
        assert!(
            sent.messages[1]
                .text
                .ends_with("You wrote:\nmy earlier idea")
        );
    }

    #[test]
    fn deleted_and_unfinished_posts_are_left_out_and_listed() {
        let me = participant("Me", ParticipantKind::Human);
        let gpt = participant("GPT", ParticipantKind::Model);
        let root = post(&me, None, "root", 1);
        let mut gone = post(&me, Some(&root), "", 2);
        gone.deleted_at = Some(UnixMillis(5));
        let mut failed = post(&gpt, Some(&gone), "partial", 3);
        failed.status = PostStatus::Failed;
        let mut deleted_ref = post(&me, Some(&root), "", 4);
        deleted_ref.deleted_at = Some(UnixMillis(6));
        let mut target = post(&me, Some(&failed), "still asking", 5);
        target.context_ids = vec![deleted_ref.id];
        let posts = vec![
            root.clone(),
            gone.clone(),
            failed.clone(),
            deleted_ref.clone(),
            target.clone(),
        ];

        let sent = build(&posts, &[me, gpt.clone()], target.id, &gpt, "m", ROOMY).unwrap();
        assert_eq!(sent.post_ids(), vec![root.id, target.id]);
        let omitted: Vec<_> = sent.omitted.iter().map(|o| (o.post_id, o.reason)).collect();
        assert_eq!(
            omitted,
            vec![
                (gone.id, OmitReason::Deleted),
                (failed.id, OmitReason::Unfinished),
                (deleted_ref.id, OmitReason::Deleted),
            ]
        );
    }

    #[test]
    fn trimming_keeps_the_first_and_last_posts_and_drops_the_oldest_middle_first() {
        let me = participant("Me", ParticipantKind::Human);
        let gpt = participant("GPT", ParticipantKind::Model);
        let long = "x".repeat(400); // about 100 tokens each
        let root = post(&me, None, &long, 1);
        let a = post(&gpt, Some(&root), &long, 2);
        let b = post(&me, Some(&a), &long, 3);
        let c = post(&gpt, Some(&b), &long, 4);
        let side = post(&me, Some(&root), &long, 5);
        let mut target = post(&me, Some(&c), "last question", 6);
        target.context_ids = vec![side.id];
        let posts = vec![
            root.clone(),
            a.clone(),
            b.clone(),
            c.clone(),
            side.clone(),
            target.clone(),
        ];
        let everyone = [me, gpt.clone()];

        let roomy = build(&posts, &everyone, target.id, &gpt, "m", ROOMY).unwrap();
        assert!(roomy.omitted.is_empty());

        let budget = roomy.estimated_tokens - 150;
        let tight = build(&posts, &everyone, target.id, &gpt, "m", budget).unwrap();
        assert!(tight.estimated_tokens <= budget);
        let trimmed: Vec<_> = tight.omitted.iter().map(|o| o.post_id).collect();
        assert_eq!(trimmed, vec![a.id, b.id], "oldest middle posts first");
        assert_eq!(tight.post_ids(), vec![root.id, c.id, side.id, target.id]);

        let tiny = build(&posts, &everyone, target.id, &gpt, "m", 10).unwrap();
        assert_eq!(
            tiny.post_ids(),
            vec![root.id, target.id],
            "context goes last; ends always stay"
        );
    }

    #[test]
    fn a_model_cannot_reply_to_an_unfinished_or_deleted_post() {
        let me = participant("Me", ParticipantKind::Human);
        let gpt = participant("GPT", ParticipantKind::Model);
        let root = post(&me, None, "root", 1);
        let mut streaming = post(&gpt, Some(&root), "half", 2);
        streaming.status = PostStatus::Streaming;
        let posts = vec![root, streaming.clone()];
        assert!(matches!(
            build(&posts, &[me, gpt.clone()], streaming.id, &gpt, "m", ROOMY),
            Err(Error::Invalid(_))
        ));
    }

    #[test]
    fn a_missing_target_is_reported() {
        assert!(matches!(
            ancestry(&[], Uuid::now_v7()),
            Err(Error::NotFound(_))
        ));
    }

    #[test]
    fn estimates_are_about_four_characters_per_token() {
        assert_eq!(estimate_tokens(""), 4);
        assert_eq!(estimate_tokens(&"a".repeat(400)), 104);
    }
}
