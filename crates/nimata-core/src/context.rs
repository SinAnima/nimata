//! What a model is shown when asked to reply: the thread from the first
//! post down to the post being replied to, and nothing else. Other branches
//! and other discussions are never included.

use std::collections::HashMap;

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
         message written by someone else starts with their name. Write your reply to the last \
         message. Do not start your reply with your own name."
    )
}

/// A request for `responder` to reply to `target`, plus the IDs of the posts
/// that were included, so the request can be recorded exactly.
///
/// Deleted posts and unfinished or failed model replies are left out; they
/// carry no usable text.
pub fn build_request(
    posts: &[Post],
    participants: &[Participant],
    target: Uuid,
    responder: &Participant,
    model: &str,
) -> Result<(ModelRequest, Vec<Uuid>)> {
    let chain = ancestry(posts, target)?;
    let last = chain.last().expect("ancestry includes the target");
    if last.deleted_at.is_some() || last.status != PostStatus::Complete {
        return Err(Error::Invalid(
            "a model can only reply to a finished post that has not been deleted".into(),
        ));
    }
    let names: HashMap<Uuid, &str> = participants
        .iter()
        .map(|p| (p.id, p.display_name.as_str()))
        .collect();

    let mut messages = Vec::new();
    let mut included = Vec::new();
    for post in chain {
        if post.deleted_at.is_some() || post.status != PostStatus::Complete {
            continue;
        }
        let message = if post.author_id == responder.id {
            Message {
                role: Role::Assistant,
                text: post.body.clone(),
            }
        } else {
            let name = names.get(&post.author_id).copied().unwrap_or("Someone");
            Message {
                role: Role::User,
                text: format!("{name}:\n{}", post.body),
            }
        };
        messages.push(message);
        included.push(post.id);
    }
    let request = ModelRequest {
        model: model.to_string(),
        instructions: instructions(&responder.display_name),
        messages,
    };
    Ok((request, included))
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
        }
    }

    #[test]
    fn only_the_thread_down_to_the_target_is_sent() {
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

        let (request, included) = build_request(
            &posts,
            &[me.clone(), gpt.clone()],
            follow_up.id,
            &gpt,
            "gpt-5.6",
        )
        .unwrap();

        assert_eq!(included, vec![question.id, answer.id, follow_up.id]);
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
        assert_eq!(request.model, "gpt-5.6");
        assert!(request.instructions.contains("You are GPT-5.6"));
    }

    #[test]
    fn deleted_and_failed_posts_are_left_out() {
        let me = participant("Me", ParticipantKind::Human);
        let gpt = participant("GPT", ParticipantKind::Model);
        let root = post(&me, None, "root", 1);
        let mut gone = post(&me, Some(&root), "", 2);
        gone.deleted_at = Some(UnixMillis(5));
        let mut failed = post(&gpt, Some(&gone), "partial", 3);
        failed.status = PostStatus::Failed;
        let target = post(&me, Some(&failed), "still asking", 4);
        let posts = vec![root.clone(), gone, failed, target.clone()];

        let (_, included) =
            build_request(&posts, &[me, gpt.clone()], target.id, &gpt, "m").unwrap();
        assert_eq!(included, vec![root.id, target.id]);
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
            build_request(&posts, &[me, gpt.clone()], streaming.id, &gpt, "m"),
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
}
