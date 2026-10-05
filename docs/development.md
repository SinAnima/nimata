# Development

## Toolchains

Toolchains are pinned with [mise](https://mise.jdx.dev) in `mise.toml`:
Rust (with iOS and Android targets), Node, Java (Android builds), the
Android command-line tools, and Ruby with CocoaPods (iOS builds).

```sh
mise install
```

Rust must run natively. On Apple Silicon, make sure rustup's default host
is `aarch64-apple-darwin` (`rustup set default-host aarch64-apple-darwin`),
otherwise mise resolves the pinned version to the x86_64 toolchain.

Android needs the NDK. `mise.toml` points `NDK_HOME` at the installed
version; install it with:

```sh
sdkmanager --sdk_root="$ANDROID_HOME" "ndk;30.0.16248370" "platforms;android-36" "build-tools;36.0.0"
```

Then install JavaScript dependencies:

```sh
npm install
```

## Running

```sh
npm run tauri dev                 # desktop app with hot reload
npm run tauri ios dev             # iOS simulator
npm run tauri android dev         # Android emulator or device
```

## Checks

Run all of these before considering a change done:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
npm run check            # svelte-check with strict TypeScript
npm run format:check     # prettier
npm test                 # vitest
npm run tauri build -- --debug
```

Coverage, with the same minimums CI enforces:

```sh
cargo llvm-cov --workspace --fail-under-lines 90   # Rust, needs cargo-llvm-cov from mise
npm run coverage                                   # frontend, thresholds in vite.config.ts
```

`actionlint` (also from mise) checks the GitHub workflow files.

Mobile build validation:

```sh
npm run tauri ios build -- --debug --target aarch64-sim
npm run tauri android build -- --debug --target aarch64 --apk
```

## Layout

```
crates/nimata-core/   domain model and logic, no Tauri dependency
src-tauri/            Tauri shell: commands, events, platform integration
src/                  Svelte UI
  lib/api.ts          the only module that calls into Rust
  lib/stores/         UI state (navigation, composer drafts, clock)
  lib/components/     UI components
docs/                 architecture, decisions
```

## Testing approach

- Rust unit tests cover domain rules.
- Repository contract tests (`crates/nimata-core/tests/`) run every case
  against both an in-memory and an on-disk database, including reopening
  and schema version handling.
- IPC tests (`src-tauri/src/ipc_tests.rs`) call commands through Tauri's
  mock runtime with the same JSON the UI sends.
- Regression tests in `src/App.test.ts` mount the whole app and drive real
  user flows: starting discussions, threaded replies, drafts surviving a
  restart, renaming, archiving, keyboard shortcuts, settings, and failure
  handling. They run the real `api.ts` against an in-memory backend
  (`src/test/fakeBackend.ts`) through Tauri's IPC mock. The fake follows the
  repository's rules; the Rust IPC tests hold the real commands to the same
  JSON shapes.
- Vitest covers stream ordering, reply references, time formatting,
  navigation, draft saving, keyboard navigation, and the composer, with
  components rendered in jsdom.
- Tauri's WebDriver tooling does not support macOS, so end-to-end flows are
  covered by the layers above plus the scripted demo for each stage.

## Inspecting local data

The database lives in the app data folder. On macOS that is
`~/Library/Application Support/org.nimata.app/nimata.sqlite3`. Settings in
the app shows the exact path. To look without risking changes:

```sh
sqlite3 -readonly ~/Library/Application\ Support/org.nimata.app/nimata.sqlite3
```

## Continuous integration

`.github/workflows/ci.yml` runs on every push and pull request on Ubuntu:
formatting, clippy, Rust tests with coverage, type checking, Prettier, and
frontend tests with coverage. Coverage reports are uploaded as a build
artifact. Releases are described in [release.md](release.md).
