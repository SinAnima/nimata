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

- Rust unit tests cover domain rules and fixture invariants.
- Vitest covers stream ordering, reply references, time formatting, and
  navigation state, with components rendered in jsdom.
- Tauri's WebDriver tooling does not support macOS, so end-to-end flows are
  covered by Rust integration tests (from Stage 1 on), component tests with
  mocked IPC, and the scripted demo for each stage.
