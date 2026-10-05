# 0003: Model providers, keys, and streaming

Status: accepted (Stage 3)

## Decisions

- **Providers live in the Rust core** (`nimata_core::providers`) behind a
  small `ModelProvider` trait: list models, and start a streamed reply. The
  trait returns boxed futures and streams so providers can be chosen at run
  time. Adapters translate to each provider's own API instead of hiding
  differences behind a lowest common denominator.
- **OpenAI uses the Responses API** with `store: false`, so the provider
  does not keep the conversation.
- **HTTP is reqwest with rustls on the `ring` backend and Mozilla's root
  certificates** (`webpki-roots`), configured explicitly. reqwest's default
  backend (aws-lc-rs) needs CMake to cross-compile, and its platform
  certificate verifier needs extra setup on Android; the chosen setup builds
  and behaves the same on desktop and mobile.
- **Keys use the `keyring` 3 crate** with the native store on each platform.
  `keyring` 4 adds an Android store and should be reconsidered in Stage 9.
- **Streaming reaches the UI as app-wide events** (`nimata://post-delta`,
  `nimata://post-updated`) rather than a per-request channel. The open
  discussion updates wherever the user navigates, and a view opened later
  reads the saved text from the database.
- **Replies are saved while they stream**, at most every 300 ms, including
  when the stream stalls, so a crash loses little. Requests still running at
  startup are marked interrupted.
- **Development conveniences** (key hints, `OPENAI_API_KEY`) are enabled by
  `cfg!(debug_assertions)` and absent from release builds.
