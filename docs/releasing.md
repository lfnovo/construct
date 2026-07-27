# Release process

Construct uses one semantic version for the desktop application and standalone
CLI. GitHub Releases is the initial canonical distribution channel. Package
managers and app stores may mirror a release later but must not rebuild or
silently replace its artifacts.

The tag-driven workflow in `.github/workflows/release.yml` creates a **draft**
release. A maintainer must inspect its assets and smoke-test them before
publication.

## Release artifacts

Each `vX.Y.Z` release candidate produces:

| Platform | Desktop application | Standalone CLI |
| --- | --- | --- |
| macOS Apple Silicon | signed-preview DMG | `construct_X.Y.Z_aarch64-apple-darwin.tar.gz` |
| macOS Intel | signed-preview DMG | `construct_X.Y.Z_x86_64-apple-darwin.tar.gz` |
| Windows x64 | NSIS setup executable | `construct_X.Y.Z_x86_64-pc-windows-msvc.zip` |

The release also contains `SHA256SUMS` for every generated installer and CLI
archive. The app and CLI are built from the same commit and Rust executable;
CLI subcommands exit before the Tauri desktop runtime starts.

The Windows preview supports the desktop workspace and stateless OKF linter.
Local knowledge indexing and MCP depend on the authenticated Unix-socket
service and are not yet available on Windows. This limitation must remain in
the release notes until a Windows IPC transport is implemented and tested.

The GitHub-generated source `.zip` and `.tar.gz` files are source archives, not
application or CLI downloads.

## Prepare

1. Start from a clean, reviewed `main`.
2. Update the version in:
   - `package.json`;
   - `package-lock.json`;
   - `src-tauri/Cargo.toml`;
   - `src-tauri/tauri.conf.json`.
3. Update `CHANGELOG.md`, user-facing documentation, and the product decision
   history.
4. Validate the intended tag and source:

```bash
npm ci
npm run release:check -- v0.1.0
npm run validate
npm run build
git status --short
```

5. Open the local `Construct.app` and smoke-test:
   - workspace restoration;
   - Markdown Preview, Edit, Review, and Source;
   - explicit save and an external-edit conflict;
   - Git Diff availability;
   - Search and OKF Explore List, Graph, and Health;
   - `construct okf lint` in text and JSON modes;
   - local MCP startup and its smoke script;
   - dark and light themes;
   - Finder and Dock icon.

## Create the release candidate

Create an annotated semantic-version tag only after the release commit passes
the local checklist:

```bash
git tag -a v0.1.0 -m "Construct 0.1.0"
git push origin v0.1.0
```

The workflow:

1. rejects a tag that disagrees with any project version source;
2. runs the complete validator;
3. builds macOS ARM, macOS Intel, and Windows x64 in isolated runners;
4. asks `tauri-action` to create or update a draft GitHub Release;
5. uploads DMG and NSIS app installers;
6. packages and uploads standalone CLI archives;
7. combines per-platform hashes into `SHA256SUMS`.

Never retag a released version. Correct a failed unpublished candidate before
publication or increment the version for any published replacement.

## Inspect the draft

Before publishing:

- confirm that every target and `SHA256SUMS` is present;
- download artifacts from the draft rather than using local build output;
- verify each checksum;
- install and launch each available app target;
- run `construct okf lint --help` and a real text/JSON lint;
- run the MCP smoke path from the standalone CLI;
- confirm the release notes and known limitations;
- confirm that no unsigned artifact is described as trusted.

Keep the release as a draft if any matrix job, signature, checksum, or smoke
test is incomplete.

## Signing and trust

The initial automated macOS preview uses ad-hoc signing. This prevents the
Apple Silicon bundle from appearing structurally unsigned, but it does not
establish developer identity or remove Gatekeeper friction.

A trusted public macOS release requires:

- an Apple Developer ID Application certificate imported into the CI keychain;
- hardened runtime and reviewed entitlements;
- notarization credentials stored as GitHub Actions secrets;
- notarization and stapling of the DMG;
- verification with `codesign`, `spctl`, and `stapler`.

A trusted public Windows release requires:

- an organization-backed code-signing certificate or signing service;
- signing of the application binary and NSIS installer;
- verification on a clean Windows machine;
- a documented response for certificate rotation and revocation.

Until those gates are configured, publish only clearly labeled preview
releases. Do not enable automatic updates before signed release identity and
rollback behavior are stable.

## Local build output

The development macOS bundle remains available at:

```text
src-tauri/target/release/bundle/macos/Construct.app
```

It is useful for local smoke tests but is not a substitute for testing the
artifact downloaded from the draft release.

## Later distribution channels

Homebrew Cask may distribute the app and a Homebrew formula may distribute the
CLI after signed releases are stable. WinGet may mirror the Windows installer.
GitHub Releases remains the immutable source of truth for versioned binaries.
