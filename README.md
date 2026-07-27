<p align="center">
  <img src="src-tauri/app-icon.svg" width="128" height="128" alt="Construct icon">
</p>

<h1 align="center">Construct</h1>

<p align="center">
  A local-first desktop knowledge workspace for coding agents.
</p>

Construct watches the project folders where you work with coding agents and gives their Markdown output a dedicated place to live. Browse recent changes, read rendered documents, edit source, compare Git changes, arrange files in panes, and explore connected [Open Knowledge Format](https://github.com/GoogleCloudPlatform/knowledge-catalog/blob/main/okf/SPEC.md) bundles.

> **Status:** early preview. Construct is useful today, but trusted signing,
> Windows hardening, and large-workspace validation are still in progress.

## Why Construct

Terminal-first agent workflows are excellent for conversation and execution, but poor at exposing the plans, specifications, reports, and knowledge files agents leave behind. Construct stays beside the terminal and turns those files into a navigable workspace without uploading their contents.

## Features

- Multiple local project folders with recursive Markdown discovery.
- Automatic filesystem monitoring and a deduplicated 30-day history.
- Tabs, horizontal and vertical panes, and workspace restoration.
- Rich Markdown editing plus editable source, with explicit saves and conflict protection.
- Document review with anchored quotes, persistent agent comments, and copyable handoffs.
- GFM preview with syntax highlighting, local images, and Mermaid.
- Read-only Git status and diff against `HEAD`.
- Dark and light themes, quick open, and Finder integration.
- Local full-text knowledge search across isolated per-folder indexes.
- Direct related-document navigation and budgeted context packs for agents.
- OKF bundle detection, metadata inspection, types, tags, links, and backlinks.
- OKF List, Graph, and Health views with multi-type filtering, stable type
  colors, actionable findings, and agent-ready lint handoffs.
- Stateless OKF linting for coding agents and CI, with text and JSON output.

## Privacy

Construct processes project files locally. It does not require an account, send document content to remote services, or write to Git. External links open only after an explicit user action.

See [SECURITY.md](SECURITY.md) for reporting and security boundaries.

## Run from source

Requirements:

- macOS 13 or newer;
- Node.js 22;
- the Rust toolchain declared in `rust-toolchain.toml`;
- Xcode Command Line Tools.

```bash
git clone https://github.com/lfnovo/construct.git
cd construct
npm ci
npm run dev
```

## Preview downloads

Tagged builds produce draft candidates in
[GitHub Releases](https://github.com/lfnovo/construct/releases):

- DMG installers for macOS Apple Silicon and Intel;
- an NSIS setup executable for Windows x64;
- standalone `construct` CLI archives for the same targets;
- a `SHA256SUMS` manifest for installer and CLI verification.

Preview artifacts are not yet a trusted public distribution: macOS
notarization and Windows code signing remain release gates. See the
[release process](docs/releasing.md) for artifact names, verification, and the
maintainer checklist.

## Validation

```bash
npm run validate
npm run build
```

The macOS bundle is written to:

```text
src-tauri/target/release/bundle/macos/Construct.app
```

## OKF linter

Validate any local OKF bundle without registering it in Construct or starting
the desktop application:

```bash
cargo run --manifest-path src-tauri/Cargo.toml -- okf lint ./knowledge
```

After a release build, use the executable inside the application bundle:

```bash
src-tauri/target/release/bundle/macos/Construct.app/Contents/MacOS/construct \
  okf lint ./knowledge
```

The default threshold fails only on conformance errors. Use
`--fail-on warning` for a strict repository gate, or `--format json` for agents
and CI. The linter is read-only and creates no workspace or index state.

Repositories that contain Markdown which is not an OKF concept can commit a
`.constructignore` at the lint root:

```gitignore
# Agent instructions and Agent Skills remain valid link targets.
AGENTS.md
CLAUDE.md
**/SKILL.md
```

These files skip OKF conformance checks but remain resolvable by internal
Markdown links. Repeated `--exclude <GLOB>` rules compose with the file;
`--no-ignore-file` performs a strict run without it.

For registered OKF Locations, open **Explore → Health** to inspect the same
native findings in the desktop application, rerun the scan, open affected
documents, or copy a bounded repair handoff for a coding agent. **Repository
policy** applies `.constructignore`; **All Markdown** exposes the strict report.

## Project documentation

- [Documentation index](docs/README.md)
- [Product specification](docs/product-spec.md)
- [Architecture](docs/architecture.md)
- [Contributing](CONTRIBUTING.md)
- [Release process](docs/releasing.md)
- [Security policy](SECURITY.md)

## Roadmap

The current priority is signed, reproducible macOS and Windows preview releases
over the same local retrieval core. Later candidates include Linux, YAML and
JSON, file management, optional local semantic search, package-manager mirrors,
and signed automatic updates.

### Local MCP access

Select a Location in Construct and use the clipboard button in the Locations
header to copy a ready-to-paste MCP configuration. The generated command grants
that server access only to the selected Location. Construct exposes local
overview, activity, search, document, link, context, and index-status tools; it
does not expose source-file mutation. Returned content is controlled by the MCP
client after it leaves Construct.

## License

Construct is available under the [MIT License](LICENSE).
