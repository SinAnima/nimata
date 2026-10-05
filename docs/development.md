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

## Running tests

All commands run from the repository root. If your shell does not activate
mise automatically, prefix them with `mise exec --`, for example
`mise exec -- npm test`.

| What                                | Command                                                         |
| ----------------------------------- | --------------------------------------------------------------- |
| Rust tests                          | `cargo test --workspace`                                        |
| Frontend tests                      | `npm test`                                                      |
| Frontend tests, re-running on save  | `npx vitest`                                                    |
| One test file                       | `npx vitest run src/App.test.ts`                                |
| Tests whose name matches            | `npx vitest run -t "drafts"`                                    |
| One Rust test                       | `cargo test --workspace drafts`                                 |
| Rust coverage, CI minimum           | `cargo llvm-cov --workspace --fail-under-lines 90`              |
| Frontend coverage, CI minimums      | `npm run coverage`                                              |
| Rust coverage report in the browser | `cargo llvm-cov --workspace --open`                             |
| Frontend coverage report            | `npm run coverage`, then open `coverage/lcov-report/index.html` |

The coverage minimums are Rust lines 90%, and frontend lines 90%,
statements 90%, functions 85%, and branches 70%. The frontend thresholds
live in `vite.config.ts`. `cargo-llvm-cov` comes from mise.

## Checks before pushing

One command runs everything CI runs, in the same way, and stops at the first
failure:

```sh
npm run verify        # or: mise run verify, or: scripts/verify.sh
```

CI calls the same script (`scripts/verify.sh`), so a local pass means the CI
checks pass. Fast checks run first so simple mistakes fail within seconds:

1. Rust formatting (`cargo fmt --all --check`)
2. Frontend formatting (`npm run format:check`)
3. Workflow files (`actionlint`)
4. Frontend build (`npm run build`, needed by the Rust crate)
5. Type check (`npm run check`)
6. Clippy (`cargo clippy --workspace --all-targets -- -D warnings`)
7. Frontend tests with coverage (`npm run coverage`)
8. Rust tests with coverage (`cargo llvm-cov --workspace --fail-under-lines 90`)

When a step fails, the script names it and prints the command to rerun
just that step. To fix formatting failures automatically, run
`cargo fmt --all` and `npm run format`.

### Run it automatically before every push

The repository includes a pre-push hook that runs the same script. Enable it
once per clone:

```sh
git config core.hooksPath .githooks
```

A failing check then cancels the push. To push anyway, for example a
work-in-progress branch, use `git push --no-verify`.

A desktop build confirms packaging still works:

```sh
npm run tauri build -- --debug
```

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

`.github/workflows/ci.yml` runs on every branch push and pull request, on
Ubuntu: formatting, clippy, Rust tests with coverage, type checking,
Prettier, and frontend tests with coverage. Coverage reports are attached to
each run as an artifact named `coverage`.

Pushing a branch is therefore enough to test CI. To follow it from the
terminal:

```sh
gh run list --branch "$(git branch --show-current)"
gh run watch                       # pick the run to follow live
gh run view --log-failed           # logs of failed steps for the latest run
```

To start CI by hand without pushing new commits:

```sh
gh workflow run CI --ref "$(git branch --show-current)"
```

Releases, and how to test the release builds before tagging, are described
in [release.md](release.md).
