# Releasing Prompt

Prompt ships as a signed macOS `.dmg` and a Homebrew cask. Releases are cut by
GitHub Actions (`.github/workflows/release.yml`); the local scripts under
`scripts/` are the same steps you can run by hand.

## Cutting a release

1. Bump `version` in the workspace `Cargo.toml` (`[workspace.package]`).
2. Merge to `main`.

The workflow notices the new version (no matching `vX.Y.Z` tag yet), tags it,
creates a GitHub Release, builds and notarizes `Prompt.dmg`, uploads it to the
release, and updates the `prompt` cask in
[`wess/homebrew-packages`](https://github.com/wess/homebrew-packages). The
version check is idempotent, so re-running is safe.

## Local build

```sh
scripts/icon.sh      # regenerate assets/icon.{png,icns} (only if the icon changed)
scripts/bundle.sh    # cargo build --release + assemble dist/Prompt.app
scripts/dmg.sh       # package dist/Prompt.dmg
```

Without `CODESIGN_IDENTITY` set, `bundle.sh` ad-hoc-signs the app: it launches on
the build machine but is not distributable. For a signed local build, set
`CODESIGN_IDENTITY` to a Developer ID Application identity from your keychain.

## Signing & notarization (CI)

Signing is optional — without secrets the workflow produces an ad-hoc build and
warns. To sign + notarize, set these repository secrets:

| Secret | What it is |
|--------|------------|
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Your Name (TEAMID)` |
| `APPLE_CERT_P12` | base64 of the exported Developer ID `.p12` |
| `APPLE_CERT_PASSWORD` | password for that `.p12` |
| `KEYCHAIN_PASSWORD` | any password for the throwaway CI keychain |
| `APPLE_ID` | Apple ID email for notarytool |
| `APPLE_TEAM_ID` | Apple Developer Team ID |
| `APPLE_APP_PASSWORD` | app-specific password for that Apple ID |
| `HOMEBREW_TAP_TOKEN` | token with write access to `wess/homebrew-packages` |

The app is signed with a hardened runtime and `assets/prompt.entitlements`
(GPUI/Metal needs the JIT / unsigned-executable-memory entitlements), then the
`.app` and `.dmg` are notarized and stapled.

## Icon

`scripts/icon.swift` draws the icon (a terminal `>_` glyph on a dark indigo
squircle) with CoreGraphics — no third-party tooling. `scripts/icon.sh` renders
the 1024px master and compiles the `.icns`. The committed `assets/icon.png` and
`assets/icon.icns` are what the bundle embeds; regenerate them only when the
design changes.
