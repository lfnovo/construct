<p align="center">
  <img src="src-tauri/app-icon.svg" width="128" height="128" alt="Construct icon">
</p>

<h1 align="center">Construct</h1>

<p align="center">
  A local-first workspace for the Markdown knowledge created by people and coding agents.
</p>

Construct turns the folders where you work with coding agents into a focused
desktop knowledge workspace. Read and edit Markdown, review documents with
agent-ready comments, follow recent changes, search across projects, and give
agents bounded read-only access to the same local knowledge.

> **Status:** early preview. The core workflows are usable, but trusted app
> signing, Windows hardening, and large-workspace validation are still in
> progress. Preview artifacts may trigger operating-system trust warnings.

## What you can do

- **Work with agent output:** monitor multiple folders, browse recent changes,
  open documents in tabs or split panes, and restore the workspace later.
- **Read and edit Markdown:** use rendered Preview, rich Edit, comment-oriented
  Review, raw Source, and read-only Git Diff while keeping saves explicit.
- **Find knowledge:** use quick file open or local full-text search over
  physically isolated per-Location indexes, then follow links or copy a bounded
  context pack.
- **Explore OKF bundles:** inspect open-ended metadata, types, tags, links,
  backlinks, graph structure, and health findings for OKF v0.1 and v0.2.
- **Support agents and CI:** run the stateless `construct okf lint` CLI or expose
  explicitly allowed Locations through the read-only local MCP server.

Construct does not require an account, upload document content, write to Git,
or hide file changes behind autosave. Markdown files remain the source of
truth.

## Get started

### Run from source

The most reliable preview path today is to run Construct from source on macOS.
You need macOS 13 or newer, Node.js 22, Xcode Command Line Tools, and `rustup`.

```bash
git clone https://github.com/lfnovo/construct.git
cd construct
npm ci
npm run dev
```

When Construct opens:

1. add a folder from the **Locations** header;
2. select a Markdown file in **Files**;
3. press `⌘P` to quick-open another file or `⌘⇧F` to search its contents;
4. switch between **Preview**, **Edit**, **Review**, **Source**, and **Diff**.

The [user guide](docs/user-guide.md) explains the workspace, editing safety,
search, OKF exploration, keyboard shortcuts, and troubleshooting.

### Preview downloads

Public preview builds are available in
[GitHub Releases](https://github.com/lfnovo/construct/releases). Releases marked
**Pre-release** are intentionally early and unsigned:

| Platform | Desktop | Standalone CLI | Local index and MCP |
| --- | --- | --- | --- |
| macOS Apple Silicon | DMG | `tar.gz` | Yes |
| macOS Intel | DMG | `tar.gz` | Yes |
| Windows x64 | NSIS installer | `.zip` | Yes |

Every candidate includes a `SHA256SUMS` manifest. The current macOS builds use
ad-hoc signing and Windows builds are not yet code-signed; trusted public
distribution is tracked in
[issue #19](https://github.com/lfnovo/construct/issues/19).

On Windows, download the asset ending in `_x64-setup.exe`. The
`x86_64-pc-windows-msvc.zip` asset is the standalone CLI, and GitHub's
automatically generated “Source code” archives are not installers. The
[user guide](docs/user-guide.md#install-a-tagged-preview) includes checksum and
Microsoft Defender SmartScreen instructions.

## CLI and agent access

Validate an OKF bundle without opening the app or creating Construct state:

```bash
construct okf lint ./knowledge
construct okf lint ./knowledge --fail-on warning --format json
```

For registered Locations, the desktop also exposes the same findings in
**Explore → Health**.

On macOS, Windows, and Unix, select a Location and use the clipboard button in the
**Locations** header to copy a ready-to-paste MCP configuration. The generated
server is read-only and limited to that Location. See:

- [CLI and OKF lint guide](docs/cli.md)
- [Local MCP guide](docs/mcp.md)

## Documentation

- [User guide](docs/user-guide.md)
- [Documentation index](docs/README.md)
- [Contributing](CONTRIBUTING.md)
- [Development guide](docs/development.md)
- [Product specification](docs/product-spec.md)
- [Architecture](docs/architecture.md)
- [Release process](docs/releasing.md)
- [Security policy](SECURITY.md)

## Development

```bash
npm ci
npm run validate
npm run build
```

The macOS app bundle is written to
`src-tauri/target/release/bundle/macos/Construct.app`.

Before contributing, read [CONTRIBUTING.md](CONTRIBUTING.md). Changes to
persistence, security boundaries, file formats, or product behavior should
start with the relevant issue or proposal.

## License

Construct is available under the [MIT License](LICENSE).
