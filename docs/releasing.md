# Release process

Construct is currently an unsigned macOS preview. This document separates reproducible build steps from the signing and distribution work still required for a public binary release.

## Prepare

1. Start from a clean `main`.
2. Update the version in `package.json`, `package-lock.json`, `src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`.
3. Update user-facing documentation and the product-spec history.
4. Run the full validator.

```bash
npm ci
npm run validate
npm run build
git status --short
```

5. Open `Construct.app` and smoke-test:
   - workspace restoration;
   - Markdown Preview and Source;
   - explicit save and an external-edit conflict;
   - Git Diff availability;
   - OKF Explore List and Graph;
   - `construct okf lint` in text and JSON modes;
   - dark and light themes;
   - Finder and Dock icon.

## Output

The unsigned development bundle is generated at:

```text
src-tauri/target/release/bundle/macos/Construct.app
```

CI may retain this bundle as an internal artifact. It must not be presented as a trusted public release until signing and notarization are configured.

## Public release prerequisites

- Apple Developer ID Application certificate;
- hardened runtime and appropriate entitlements;
- notarization credentials stored as GitHub Actions secrets;
- signed and notarized application or disk image;
- verification with `codesign`, `spctl`, and `stapler`;
- a documented rollback and update strategy.

## Version tags

Use semantic versions and annotated tags after the release commit:

```bash
git tag -a v0.1.0 -m "Construct 0.1.0"
git push origin main --follow-tags
```

Do not create a release tag until the corresponding artifact has passed the smoke checklist.
