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
   - macOS: a universal `.dmg` and `.app.tar.gz` for Apple silicon and
     Intel.
   - Windows: `.msi` and `-setup.exe` (NSIS) installers.
5. Publishes the release only after every build succeeded.

If any step fails, the draft remains for inspection and can be deleted from
the Releases page before re-tagging.

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
