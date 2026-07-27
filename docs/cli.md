# CLI and OKF lint

**Status:** Current preview behavior

The `construct` executable contains both the desktop runtime and stateless CLI
subcommands. `construct okf lint` validates a local Open Knowledge Format
bundle without registering a Location, opening the desktop, starting the
retrieval service, or creating Construct state.

The linter is read-only. It never repairs, rewrites, or normalizes Markdown.

## Get the executable

### Standalone release archive

Tagged release candidates provide:

- `construct_X.Y.Z_aarch64-apple-darwin.tar.gz`;
- `construct_X.Y.Z_x86_64-apple-darwin.tar.gz`;
- `construct_X.Y.Z_x86_64-pc-windows-msvc.zip`.

Download the matching archive and `SHA256SUMS` from
[GitHub Releases](https://github.com/lfnovo/construct/releases), verify it, and
place `construct` or `construct.exe` somewhere on your `PATH`.

Preview artifacts are not yet a trusted signed distribution. Review the
[release guide](releasing.md) before using them in an automated environment.

### Build from source

```bash
git clone https://github.com/lfnovo/construct.git
cd construct
cargo build --release --manifest-path src-tauri/Cargo.toml
```

The executable is:

```text
src-tauri/target/release/construct
```

On Windows it is `src-tauri\target\release\construct.exe`.

A Tauri macOS bundle also contains the same executable:

```text
Construct.app/Contents/MacOS/construct
```

For repeated use, prefer the standalone archive or put a stable symlink on your
`PATH` instead of typing a build-directory path:

```bash
mkdir -p ~/.local/bin
ln -s /absolute/path/to/construct ~/.local/bin/construct
```

Ensure `~/.local/bin` is on your shell's `PATH`.

## Run a lint

```bash
construct okf lint ./knowledge
```

The path defaults to the current directory:

```bash
cd ./knowledge
construct okf lint
```

The complete command is:

```text
Usage: construct okf lint [PATH] [OPTIONS]

Arguments:
  PATH                       Bundle directory (default: current directory)

Options:
  --format <text|json>       Output format (default: text)
  --fail-on <error|warning|never>
                             Finding threshold for exit code 1 (default: error)
  --exclude <GLOB>           Skip conformance checks for a path (repeatable)
  --no-ignore-file           Do not read .constructignore from the bundle root
  --max-findings <COUNT>     Maximum findings included in output (default: 1000)
  --no-color                 Disable terminal colors
  --quiet                    Suppress individual findings
  -h, --help                 Show this help
```

Use `construct okf lint --help` for the version installed on your machine.
The root command does not currently expose a general `construct --help`
screen.

## Failure thresholds and exit codes

The default threshold fails only on conformance errors:

```bash
construct okf lint ./knowledge --fail-on error
```

Use warnings as a stricter repository gate:

```bash
construct okf lint ./knowledge --fail-on warning
```

Use `never` for an informational report that still distinguishes invocation
errors:

```bash
construct okf lint ./knowledge --fail-on never
```

Public exit codes are:

| Code | Meaning |
| --- | --- |
| `0` | Scan completed with no finding at the configured failure threshold |
| `1` | Scan completed and a finding met the configured threshold |
| `2` | Invalid invocation or runtime failure |

`--max-findings` limits output only. The scan, summary counts, and exit decision
still consider every finding.

## Text and JSON output

Text output is designed for people and terminal logs:

```bash
construct okf lint ./knowledge --no-color
```

JSON is a single versioned object for agents and CI:

```bash
construct okf lint ./knowledge --format json
```

It includes:

- schema and tool versions;
- bundle name and declared OKF version when present;
- document, ignored-document, severity, and truncation counts;
- deterministic findings with code, severity, tier, relative path, source range,
  and English message.

Normal output contains relative paths only.

Use `--quiet` when a summary and exit code are enough:

```bash
construct okf lint ./knowledge --quiet --fail-on warning
```

## Repository policy with `.constructignore`

An OKF repository often contains Markdown that is useful but is not an OKF
concept, such as agent instructions or skill definitions. Commit a
`.constructignore` at the bundle root to omit those files from conformance
checks:

```gitignore
# Agent instructions remain valid Markdown link targets.
AGENTS.md
CLAUDE.md
**/SKILL.md

# Ignore a generated documentation subtree.
generated/**

# Reinclude one reviewed file.
!generated/canonical.md
```

Rules:

- blank lines and `#` comments are ignored;
- patterns are evaluated in order;
- `!` negation can reinclude a path;
- ignored Markdown produces no findings and is not projected as an OKF concept;
- ignored Markdown remains available as an internal-link target;
- the policy does not delete files or remove them from Construct's general
  Markdown search.

Add temporary command-line exclusions after repository rules:

```bash
construct okf lint . \
  --exclude 'drafts/**' \
  --exclude '**/fixtures/**'
```

Bypass `.constructignore` for a strict audit:

```bash
construct okf lint . --no-ignore-file
```

Traversal still excludes standard dependency, cache, version-control, and build
directories. Directory symlinks are not followed.

## CI examples

### Build and lint from the repository source

This is the safest option before trusted standalone releases are available:

```yaml
name: OKF

on:
  pull_request:
  push:
    branches: [main]

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Check out a pinned Construct revision
        uses: actions/checkout@v4
        with:
          repository: lfnovo/construct
          ref: 2b4b121b9e56be5c4e652d9f8250190552f8d978
          path: .construct-tool

      - uses: dtolnay/rust-toolchain@1.97.1

      - name: Check OKF conformance
        run: >-
          cargo run --quiet --locked
          --manifest-path .construct-tool/src-tauri/Cargo.toml
          --
          okf lint .
          --fail-on warning
          --no-color
          --exclude '.construct-tool/**'
```

Update the full SHA deliberately when adopting a newer reviewed Construct
revision. A release tag is easier to read, but a full commit SHA gives the
strongest pin. The explicit exclusion keeps the checked-out tool source outside
the bundle's conformance report.

### Use a verified standalone binary

When trusted release artifacts are published:

1. pin a specific Construct version;
2. download the matching CLI archive and `SHA256SUMS`;
3. verify the archive checksum;
4. extract the binary;
5. run `construct okf lint`.

Do not download an unpinned “latest” binary in CI.

## Desktop equivalent

For a registered OKF Location, **Explore → Health** uses the same native parser
and finding codes:

- **Repository policy** applies `.constructignore`;
- **All Markdown** bypasses it;
- **Run lint** refreshes saved-file findings;
- findings can open the affected document in Source;
- the report can be copied as a bounded agent repair handoff.

The desktop view does not spawn the CLI and does not repair files.

## Current boundaries

The first linter release intentionally has no:

- auto-fix or source mutation;
- watch mode or cache;
- profiles or closed type vocabulary;
- SARIF output;
- network access;
- dependency on the desktop workspace, local index, or MCP server.

Windows supports the same stateless linter, local indexing, and MCP executable
as the desktop preview.
