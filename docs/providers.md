# Model providers

Models take part in Nimata discussions as participants. A provider adapter
connects Nimata to a model service; the discussion itself always stays in
Nimata's database.

## Supported

| Provider                      | API used                                                                    | Key      |
| ----------------------------- | --------------------------------------------------------------------------- | -------- |
| OpenAI                        | Responses API (`POST /responses`, streaming, `store: false`), `GET /models` | Required |
| Anthropic                     | Messages API (`POST /messages`, streaming), `GET /models`                   | Required |
| OpenAI-compatible connections | Chat Completions (`POST /chat/completions`, streaming), `GET /models`       | Optional |

OpenAI-compatible connections cover local runners (Ollama at
`http://localhost:11434/v1`, LM Studio at `http://localhost:1234/v1`,
llama.cpp, vLLM) and hosted services that speak the same API (for example
xAI or Qwen's compatible endpoints). Add as many as you like under
**Add an OpenAI-compatible connection…**; each has its own name, endpoint
address, optional key, and models. Gemini will get its own adapter later.

Providers differ, and Nimata does not hide that: each kind declares what it
can do (whether it needs a key, whether its endpoint can change, whether it
reports token counts), and Settings offers only what applies.

### Provider-specific details

- **Anthropic:** replies may use up to 32,000 output tokens; a model that
  allows fewer is asked again once with 8,192. Thinking is not shown, only
  the reply text. A conversation must start and end with the user, so when
  a thread starts with, or ends at, the model's own post, Nimata adds a
  short bridging line. If Claude declines to answer, the reply fails with
  "The model declined to answer this" and the partial text is not treated
  as an answer.
- **OpenAI-compatible:** Nimata asks for token counts
  (`stream_options.include_usage`); a server that rejects that option is
  asked again without it. Messages name the connection, for example "The
  model qwen3:8b is not available at Ollama."

## Setting up a model

Settings, Models:

1. Paste an API key and choose **Save key**.
2. **Test connection** lists the provider's text models, which also proves
   the key works.
3. Choose a model and **Add model**. It becomes a participant with an
   editable name (for example "GPT-5.6"). Turning it off hides it from
   **Ask** without changing the replies it already wrote.

## Asking a model to reply

Posting asks a model to answer, so a conversation flows without extra
clicks:

1. **Who you reply to.** The composer replies to the newest post (one that
   is not deleted and did not fail). Choose **Reply** on another post to
   reply there instead; **Esc** or **Reply to latest** goes back. Every
   post after a discussion's first replies to something; start a new
   discussion for a new topic.
2. **Who answers**, in this order:
   - models you **@mention** in the text, each answering as its own reply;
   - the model you pick under **Then ask** (or **No one**);
   - the model whose post you are replying to;
   - the model that last replied in the discussion;
   - the default model chosen in Settings, Models.

   The line under the composer always says who will answer before you post.

Every finished post also has **Ask GPT-5.6** (or **Ask…** with several
models, including **All of them**) to ask a model directly. When several
models are asked, each answers as its own reply to the same post; one
failing never affects the others. The reply appears straight away as a new
post and its text streams in. **Stop** ends it early and keeps the text so
far; **Retry** on a stopped or failed reply asks again as a new reply,
keeping the old one.

## Mentions and aliases

Every model can be mentioned by its automatic alias: its name in lowercase
letters and digits, e.g. GPT-6-sol is `@gpt6sol`. Add your own aliases in
Settings, Models, e.g. `review, r`, so that `@review what do you think`
asks that model.

- Aliases are case-insensitive and each names exactly one model.
- A shorter prefix works when only one model has an alias starting with
  it: `@cl` for `@claude`. An exact alias wins over longer ones, so `@r`
  can name one model even if another has `@research`.
- A mention that matches several models, or none, is pointed out before
  posting, and posting waits until it is fixed.
- Typing `@` offers matching models; **Tab** completes when only one
  matches.
- E-mail addresses are not mentions.

Aliases that ask a group of models at once come with model groups in a
later stage.

## What a model is sent

1. **The reply chain:** the posts from the discussion's first post down to
   the post being answered. Other branches are not sent.
2. **Context you chose:** posts marked with **Include** (or C) while
   writing are saved with your post as context references. They are sent
   in a separate message labelled "For context, from another branch of this
   discussion (not part of the reply chain):", placed just before the post
   that chose them. References carry down the thread: any model replying
   further down that chain sees them too.
3. **Files attached to those posts:** text files inline, images and PDFs
   where the provider accepts them; a file a model cannot read is named
   but not sent. See [attachments.md](attachments.md#what-models-receive).
4. **A short instruction** saying the model is one participant in a
   threaded discussion, that context is labelled, and that it should reply
   to the last message.

Deleted posts and unfinished or failed replies are left out. Each message
from someone else starts with their name; the model's own earlier posts are
sent as its own messages ("You wrote:" inside context). Other discussions
are never included.

**Size and trimming.** Nimata estimates the size at about four characters
per token and keeps each request within a budget: 100,000 estimated tokens
for OpenAI and Anthropic, 6,000 for OpenAI-compatible connections, whose
local models often have small context windows. Over budget, the oldest
posts in the middle of the chain are left out first, never the first post
or the post being answered; then context, oldest first.

**Before and after.** **What will be sent?** under the composer shows
exactly what the answering model will get, including what is left out and
why. After a reply, its details show **See exactly what was sent**: the
exact instructions and messages are recorded with each request, so the
answer stays available even if posts are later edited or deleted.

## What is recorded

The reply is an ordinary Nimata post written by the model participant, with
a Nimata ID. Provider details are subordinate metadata on the post:
provider, the exact model version that answered, the provider's response ID
and request ID, token counts, and whether the reply stopped early. They are
never used as Nimata's identity for anything.

A `generations` record per request holds its status (`queued`, `sending`,
`streaming`, `complete`, `failed`, `cancelled`), any error, and the posts
sent.

## Failures

Provider failures never damage the discussion. The reply post keeps the
text received and is marked failed, with a message written for people:

| Situation                                 | Message                                                                 |
| ----------------------------------------- | ----------------------------------------------------------------------- |
| Key rejected (401)                        | OpenAI rejected the API key. Check it in Settings, Models.              |
| Not allowed (403)                         | This API key is not allowed to use the model.                           |
| Unknown model (404)                       | The model is not available to this API key.                             |
| Out of credit (429, `insufficient_quota`) | The OpenAI account has run out of credit or reached its spending limit. |
| Rate limited (429)                        | OpenAI is limiting requests right now. Wait a moment, then retry.       |
| Server error (5xx)                        | OpenAI had a server problem. Retry in a moment.                         |
| No connection                             | Could not reach OpenAI. Check the internet connection, then retry.      |
| Stream cut off                            | The connection to OpenAI ended before the reply was finished.           |

The provider's own wording is appended in parentheses when it adds
something.

If Nimata quits while a reply is streaming, the text received so far has
been saved (at most 300 ms is lost), and on the next launch the reply is
marked "Nimata was closed before this reply was finished", with Retry
available.

## API keys

See [privacy.md](privacy.md#api-keys).

## Testing

Provider tests replay recorded streams from a local HTTP server, so they
need no network or key. One live test talks to the real API and is skipped
unless run explicitly:

```sh
OPENAI_API_KEY=sk-... cargo test -p nimata-core --test openai_live -- --ignored
```

`NIMATA_LIVE_MODEL` chooses the model; otherwise the first "mini" model is
used. It costs a fraction of a cent.
