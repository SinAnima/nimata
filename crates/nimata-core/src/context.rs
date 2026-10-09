//! What a model is shown when asked to reply.
//!
//! By default: the reply chain from the first post down to the post being
//! answered. In addition, posts that anyone in that chain chose as context
//! ("Include as context") are carried down to every later reply, and sent in
//! a labelled message so the model can tell them apart from the chain.
//! Other discussions are never included.
//!
//! Files attached to those posts go with them: text files inline, images
//! and PDFs as files when the provider accepts them. A file a model cannot
//! take is named in the text, so it knows the file exists, and listed as
//! left out.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::attachments::{Attachment, AttachmentKind};
use crate::domain::{Participant, Post, PostStatus};
use crate::error::{Error, Result};
use crate::providers::{FileKind, FilePart, FileSupport, Message, ModelRequest, Role};

/// The longest text file sent inline, in characters.
pub const MAX_INLINE_TEXT_CHARS: usize = 200_000;

/// The files attached to the posts, and what the model accepts.
#[derive(Debug, Default, Clone, Copy)]
pub struct Files<'a> {
    pub attachments: &'a [Attachment],
    /// Contents of text attachments, by content hash. A text file whose
    /// contents are not here is reported as not available.
    pub texts: Option<&'a HashMap<String, String>>,
    pub support: FileSupport,
}

/// How an attached file reached the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Delivery {
    /// Inline, as part of the message text.
    Text,
    Image,
    Pdf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SentAttachment {
    pub id: Uuid,
    pub filename: String,
    pub media_type: String,
    pub size: u64,
    pub content_hash: String,
    pub delivery: Delivery,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileOmitReason {
    /// The model cannot read this kind of file.
    Unsupported,
    TooLarge,
    /// The file's bytes are not on this device.
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OmittedAttachment {
    pub id: Uuid,
    pub post_id: Uuid,
    pub filename: String,
    pub reason: FileOmitReason,
}

/// What one post's files add to its message.
#[derive(Default)]
struct PostFiles {
    text: String,
    sent: Vec<SentAttachment>,
    omitted: Vec<OmittedAttachment>,
    tokens: u64,
}

/// A rough token cost for a file sent as a file: images are scaled by the
/// provider to about 1,600 tokens at most; PDFs cost per page, which is
/// guessed from the size.
fn file_tokens(delivery: Delivery, size: u64) -> u64 {
    match delivery {
        Delivery::Text => 0,
        Delivery::Image => 1_600,
        Delivery::Pdf => (size / 20).max(1_500),
    }
}

/// A file size for people, e.g. "2.3 KB".
pub fn human_size(bytes: u64) -> String {
    match bytes {
        b if b < 1024 => format!("{b} bytes"),
        b if b < 1024 * 1024 => format!("{:.1} KB", b as f64 / 1024.0),
        b => format!("{:.1} MB", b as f64 / (1024.0 * 1024.0)),
    }
}

fn post_files(files: &Files, post_id: Uuid) -> PostFiles {
    let mut out = PostFiles::default();
    for a in files.attachments.iter().filter(|a| a.post_id == post_id) {
        let sent = |delivery| SentAttachment {
            id: a.id,
            filename: a.filename.clone(),
            media_type: a.media_type.clone(),
            size: a.size,
            content_hash: a.content_hash.clone(),
            delivery,
        };
        let support = files.support;
        let decision = match a.kind {
            AttachmentKind::Text => match files.texts.and_then(|t| t.get(&a.content_hash)) {
                None => Err(FileOmitReason::Missing),
                Some(text) if text.chars().count() > MAX_INLINE_TEXT_CHARS => {
                    Err(FileOmitReason::TooLarge)
                }
                Some(text) => {
                    out.text.push_str(&format!(
                        "\n\n<attachment name=\"{}\" type=\"{}\">\n{}\n</attachment>",
                        a.filename,
                        a.media_type,
                        text.trim_end()
                    ));
                    Ok(Delivery::Text)
                }
            },
            AttachmentKind::Image if !support.images => Err(FileOmitReason::Unsupported),
            AttachmentKind::Image if a.size > support.max_image_bytes => {
                Err(FileOmitReason::TooLarge)
            }
            AttachmentKind::Image => {
                out.text
                    .push_str(&format!("\n\n[Attached image: {}]", a.filename));
                Ok(Delivery::Image)
            }
            AttachmentKind::Pdf if !support.pdfs => Err(FileOmitReason::Unsupported),
            AttachmentKind::Pdf if a.size > support.max_pdf_bytes => Err(FileOmitReason::TooLarge),
            AttachmentKind::Pdf => {
                out.text
                    .push_str(&format!("\n\n[Attached PDF: {}]", a.filename));
                Ok(Delivery::Pdf)
            }
            AttachmentKind::Other => Err(FileOmitReason::Unsupported),
        };
        match decision {
            Ok(delivery) => {
                out.tokens += file_tokens(delivery, a.size);
                out.sent.push(sent(delivery));
            }
            Err(reason) => {
                let why = match reason {
                    FileOmitReason::Unsupported => "You cannot read this kind of file.",
                    FileOmitReason::TooLarge => "It is too large to send.",
                    FileOmitReason::Missing => "It is not available on this device.",
                };
                out.text.push_str(&format!(
                    "\n\n[Attached file not included: {} ({}, {}). {why}]",
                    a.filename,
                    a.media_type,
                    human_size(a.size)
                ));
                out.omitted.push(OmittedAttachment {
                    id: a.id,
                    post_id,
                    filename: a.filename.clone(),
                    reason,
                });
            }
        }
    }
    out
}

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
    /// Files sent with the message; text files are inside `text`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<SentAttachment>,
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
    /// Files on posts that were sent, but which this model could not take.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub omitted_attachments: Vec<OmittedAttachment>,
}

impl SentContext {
    /// The request for the provider adapter. `read` returns the bytes of
    /// an attached file by content hash; it is called only for images and
    /// PDFs being sent.
    pub fn request(&self, read: impl Fn(&str) -> Result<Vec<u8>>) -> Result<ModelRequest> {
        let messages = self
            .messages
            .iter()
            .map(|m| {
                let files = m
                    .attachments
                    .iter()
                    .filter_map(|a| {
                        let kind = match a.delivery {
                            Delivery::Text => return None,
                            Delivery::Image => FileKind::Image,
                            Delivery::Pdf => FileKind::Pdf,
                        };
                        Some(read(&a.content_hash).map(|data| FilePart {
                            filename: a.filename.clone(),
                            media_type: a.media_type.clone(),
                            kind,
                            data,
                        }))
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok(Message {
                    role: m.role,
                    text: m.text.clone(),
                    files,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(ModelRequest {
            model: self.model.clone(),
            instructions: self.instructions.clone(),
            messages,
        })
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
    files: Files,
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

    let attached: HashMap<Uuid, PostFiles> = posts
        .iter()
        .filter(|p| files.attachments.iter().any(|a| a.post_id == p.id))
        .map(|p| (p.id, post_files(&files, p.id)))
        .collect();
    let empty = PostFiles::default();
    let files_of = |post: &Post| attached.get(&post.id).unwrap_or(&empty);

    let render = |entry: &Entry| -> SentMessage {
        match entry {
            Entry::Thread(post) if post.author_id == responder.id => SentMessage {
                role: Role::Assistant,
                text: post.body.clone(),
                source: Source::Thread,
                post_ids: vec![post.id],
                attachments: vec![],
            },
            Entry::Thread(post) => SentMessage {
                role: Role::User,
                text: format!("{}:\n{}{}", name(post), post.body, files_of(post).text),
                source: Source::Thread,
                post_ids: vec![post.id],
                attachments: files_of(post).sent.clone(),
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
                        format!("{who} wrote:\n{}{}", p.body, files_of(p).text)
                    })
                    .collect();
                SentMessage {
                    role: Role::User,
                    text: format!("{CONTEXT_LABEL}\n\n{}", parts.join("\n\n")),
                    source: Source::Context,
                    post_ids: posts.iter().map(|p| p.id).collect(),
                    attachments: posts
                        .iter()
                        .flat_map(|p| files_of(p).sent.clone())
                        .collect(),
                }
            }
        }
    };

    let instructions = instructions(&responder.display_name);
    let cost = |entry: &Entry| -> u64 {
        let message = render(entry);
        let file_cost: u64 = message
            .attachments
            .iter()
            .map(|a| file_tokens(a.delivery, a.size))
            .sum();
        estimate_tokens(&message.text) + file_cost
    };
    let total = |entries: &[Entry]| -> u64 {
        estimate_tokens(&instructions) + entries.iter().map(cost).sum::<u64>()
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
    let omitted_attachments = messages
        .iter()
        .filter(|m| m.role == Role::User)
        .flat_map(|m| m.post_ids.iter())
        .flat_map(|id| {
            attached
                .get(id)
                .map(|f| f.omitted.clone())
                .unwrap_or_default()
        })
        .collect();
    Ok(SentContext {
        model: model.to_string(),
        instructions,
        messages,
        omitted,
        estimated_tokens,
        budget_tokens,
        omitted_attachments,
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
            Files::default(),
        )
        .unwrap();

        assert_eq!(sent.post_ids(), vec![question.id, answer.id, follow_up.id]);
        let request = sent.request(|_| unreachable!("no files")).unwrap();
        assert_eq!(
            request.messages,
            vec![
                Message {
                    role: Role::User,
                    text: "Thanos:\nCould Datalog replace our mapping engine?".into(),
                    files: vec![]
                },
                Message {
                    role: Role::Assistant,
                    text: "Yes, with caveats.".into(),
                    files: vec![]
                },
                Message {
                    role: Role::User,
                    text: "Thanos:\nWhat about overrides?".into(),
                    files: vec![]
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

        let sent = build(
            &posts,
            &everyone,
            mine.id,
            &grok,
            "grok-5",
            ROOMY,
            Files::default(),
        )
        .unwrap();

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

        let sent = build(
            &posts,
            &[me, gpt.clone()],
            later.id,
            &gpt,
            "m",
            ROOMY,
            Files::default(),
        )
        .unwrap();
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
        let sent = build(
            &posts,
            &[me, gpt.clone()],
            mine.id,
            &gpt,
            "m",
            ROOMY,
            Files::default(),
        )
        .unwrap();
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

        let sent = build(
            &posts,
            &[me, gpt.clone()],
            target.id,
            &gpt,
            "m",
            ROOMY,
            Files::default(),
        )
        .unwrap();
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

        let roomy = build(
            &posts,
            &everyone,
            target.id,
            &gpt,
            "m",
            ROOMY,
            Files::default(),
        )
        .unwrap();
        assert!(roomy.omitted.is_empty());

        let budget = roomy.estimated_tokens - 150;
        let tight = build(
            &posts,
            &everyone,
            target.id,
            &gpt,
            "m",
            budget,
            Files::default(),
        )
        .unwrap();
        assert!(tight.estimated_tokens <= budget);
        let trimmed: Vec<_> = tight.omitted.iter().map(|o| o.post_id).collect();
        assert_eq!(trimmed, vec![a.id, b.id], "oldest middle posts first");
        assert_eq!(tight.post_ids(), vec![root.id, c.id, side.id, target.id]);

        let tiny = build(
            &posts,
            &everyone,
            target.id,
            &gpt,
            "m",
            10,
            Files::default(),
        )
        .unwrap();
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
            build(
                &posts,
                &[me, gpt.clone()],
                streaming.id,
                &gpt,
                "m",
                ROOMY,
                Files::default()
            ),
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

    fn attachment(post: &Post, name: &str, kind: AttachmentKind, size: u64) -> Attachment {
        let media_type = match kind {
            AttachmentKind::Text => "text/markdown",
            AttachmentKind::Image => "image/png",
            AttachmentKind::Pdf => "application/pdf",
            AttachmentKind::Other => "application/zip",
        };
        Attachment {
            id: Uuid::now_v7(),
            post_id: post.id,
            filename: name.into(),
            media_type: media_type.into(),
            size,
            content_hash: crate::attachments::content_hash(name.as_bytes()),
            kind,
            created_at: UnixMillis(0),
        }
    }

    #[test]
    fn files_go_to_models_that_can_read_them_and_are_named_otherwise() {
        let me = participant("Thanos", ParticipantKind::Human);
        let claude = participant("Claude", ParticipantKind::Model);
        let question = post(&me, None, "What do you make of these?", 1);
        let files = vec![
            attachment(&question, "notes.md", AttachmentKind::Text, 7),
            attachment(&question, "diagram.png", AttachmentKind::Image, 2_000),
            attachment(&question, "paper.pdf", AttachmentKind::Pdf, 100_000),
            attachment(
                &question,
                "data.zip",
                AttachmentKind::Other,
                3 * 1024 * 1024,
            ),
        ];
        let texts = HashMap::from([(files[0].content_hash.clone(), "# Notes\n".to_string())]);
        let posts = vec![question.clone()];
        let everyone = [me.clone(), claude.clone()];
        let ask = |support| {
            let files = Files {
                attachments: &files,
                texts: Some(&texts),
                support,
            };
            build(&posts, &everyone, question.id, &claude, "m", ROOMY, files).unwrap()
        };

        // A model that takes images and PDFs.
        let rich = ask(crate::providers::file_support(
            crate::domain::ProviderKind::Anthropic,
        ));
        let message = &rich.messages[0];
        assert_eq!(
            message.text,
            "Thanos:\nWhat do you make of these?\
             \n\n<attachment name=\"notes.md\" type=\"text/markdown\">\n# Notes\n</attachment>\
             \n\n[Attached image: diagram.png]\
             \n\n[Attached PDF: paper.pdf]\
             \n\n[Attached file not included: data.zip (application/zip, 3.0 MB). You cannot read this kind of file.]"
        );
        let deliveries: Vec<Delivery> = message.attachments.iter().map(|a| a.delivery).collect();
        assert_eq!(
            deliveries,
            vec![Delivery::Text, Delivery::Image, Delivery::Pdf]
        );
        assert_eq!(rich.omitted_attachments.len(), 1);
        assert_eq!(
            rich.omitted_attachments[0].reason,
            FileOmitReason::Unsupported
        );
        // Files count towards the size: 1,600 for the image, 5,000 for the PDF.
        assert!(rich.estimated_tokens > 6_600);

        let read = |hash: &str| Ok(hash.as_bytes().to_vec());
        let request = rich.request(read).unwrap();
        let parts: Vec<(&str, FileKind)> = request.messages[0]
            .files
            .iter()
            .map(|f| (f.filename.as_str(), f.kind))
            .collect();
        assert_eq!(
            parts,
            vec![
                ("diagram.png", FileKind::Image),
                ("paper.pdf", FileKind::Pdf)
            ]
        );

        // A text-only model still gets the text file, and is told of the rest.
        let plain = ask(FileSupport::default());
        assert!(
            plain.messages[0]
                .text
                .contains("<attachment name=\"notes.md\"")
        );
        assert!(
            plain.messages[0]
                .text
                .contains("[Attached file not included: diagram.png")
        );
        assert_eq!(plain.omitted_attachments.len(), 3);
        assert!(
            plain
                .request(|_| unreachable!("nothing to read"))
                .unwrap()
                .messages[0]
                .files
                .is_empty()
        );

        // Too large, or not on this device.
        let small = FileSupport {
            max_image_bytes: 1_000,
            ..crate::providers::file_support(crate::domain::ProviderKind::OpenAi)
        };
        let reasons: Vec<FileOmitReason> = ask(small)
            .omitted_attachments
            .iter()
            .map(|o| o.reason)
            .collect();
        assert_eq!(
            reasons,
            vec![FileOmitReason::TooLarge, FileOmitReason::Unsupported]
        );
        let none = Files {
            attachments: &files[..1],
            texts: None,
            support: FileSupport::default(),
        };
        let missing = build(&posts, &everyone, question.id, &claude, "m", ROOMY, none).unwrap();
        assert_eq!(
            missing.omitted_attachments[0].reason,
            FileOmitReason::Missing
        );
        assert!(
            missing.messages[0]
                .text
                .contains("It is not available on this device.")
        );
    }
}
