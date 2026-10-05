# Releasing

Pushing a tag of the form `vMAJOR.MINOR.PATCH` builds Nimata for macOS and
Windows and publishes the installers as a GitHub Release.

## Steps

1. Set the same version in `Cargo.toml` (`[workspace.package] version`) and
   `package.json`. The app version comes from `Cargo.toml`.
2. Commit, then tag and push:

   ```sh
   git tag v0.2.0
   git push origin v0.2.0
   ```

## What the workflow does

`.github/workflows/release.yml`:

1. Runs the full CI workflow (lint, tests, coverage thresholds).
2. Checks that the tag matches the version in `Cargo.toml` and
   `package.json`, and stops if it does not.
3. Creates a draft GitHub Release with generated notes.
4. Builds in parallel and uploads to the draft:
   - macOS: a universal `.dmg` for Apple silicon and Intel.
   - Windows: `.msi` and `-setup.exe` (NSIS) installers.
5. Publishes the release only after every build succeeded.

If any step fails, the draft remains for inspection and can be deleted from
the Releases page before re-tagging.

## Testing the release builds before tagging

The release workflow can also be started by hand on any branch. A manual
run does everything except publishing:

1. Runs the full CI workflow.
2. Builds the macOS and Windows apps exactly as a tagged release would.
3. Attaches the installers to the run as downloadable artifacts
   (`nimata-macOS` and `nimata-Windows`).

It skips the version check and does not create, change, or publish any
GitHub Release, so it is safe to run as often as needed.

```sh
gh workflow run Release --ref "$(git branch --show-current)"
gh run watch                                   # follow it; builds take several minutes
gh run download --name nimata-macOS            # into the current folder
gh run download --name nimata-Windows
```

`gh run download` takes the most recent run unless given a run ID
(`gh run download <run-id> --name nimata-macOS`). The same files are on the
run's page on GitHub, under Artifacts. You can also start the workflow from
the Actions tab: choose Release, then Run workflow, and pick the branch.

Install and launch the downloaded apps (see Unsigned builds below), then tag
once they work. A typical sequence:

1. Push the branch and wait for CI to pass.
2. Run the Release workflow manually and try the installers.
3. Merge, bump the version if needed, then tag `vX.Y.Z` and push the tag.

The macOS and Windows jobs cannot be run locally: GitHub-hosted runners are
the only way to build both platforms from one machine. Locally you can only
reproduce the macOS build, with `npm run tauri build -- --target
universal-apple-darwin` after `rustup target add x86_64-apple-darwin`.

## Unsigned builds

The builds are not code-signed yet.

- **macOS**: Gatekeeper blocks the first launch. Right-click Nimata in
  Applications, choose Open, and confirm. Alternatively run
  `xattr -dr com.apple.quarantine /Applications/Nimata.app`.
- **Windows**: SmartScreen warns about an unrecognised app. Choose More
  info, then Run anyway.

To sign and notarise macOS builds later, add the Apple certificate and
notarisation credentials as repository secrets and pass them to the
`tauri-action` step as `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD`,
`APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD`, and `APPLE_TEAM_ID`.
See the Tauri distribution guides for macOS and Windows signing.
