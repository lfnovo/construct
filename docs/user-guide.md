# Construct user guide

**Status:** Current preview behavior

Construct is a local-first workspace for Markdown files created and used by
people and coding agents. It monitors folders you choose, keeps the files as the
source of truth, and adds reading, editing, review, search, history, OKF, and
agent-access workflows around them.

## Before you install

The preview currently has these platform boundaries:

| Capability | macOS | Windows x64 |
| --- | --- | --- |
| Desktop Markdown workspace | Yes | Preview |
| Stateless `construct okf lint` CLI | Yes | Yes |
| Local full-text index | Yes | Yes |
| Local MCP server | Yes | Yes |

Linux is not currently part of the release matrix. The retrieval service and
MCP use authenticated Unix-domain socket IPC on macOS/Unix and local named
pipes on Windows.

Preview downloads are not yet a trusted public distribution. macOS artifacts
use ad-hoc signing and are not notarized, and Windows artifacts are not yet
code-signed. Build from source if you do not want to use an untrusted preview
artifact. Trusted signing work is tracked in
[issue #19](https://github.com/lfnovo/construct/issues/19).

## Install and open Construct

### Run from source on macOS

Install:

- macOS 13 or newer;
- Node.js 22;
- Xcode Command Line Tools;
- `rustup` (the repository selects the pinned Rust toolchain).

Then run:

```bash
git clone https://github.com/lfnovo/construct.git
cd construct
npm ci
npm run dev
```

To create a release-mode application bundle:

```bash
npm run build
open src-tauri/target/release/bundle/macos/Construct.app
```

### Install a tagged preview

Published preview releases are public at
[GitHub Releases](https://github.com/lfnovo/construct/releases) and carry a
**Pre-release** label. A URL containing `untagged-...` points to a private draft
for maintainers and will not work for other users.

For Windows x64:

1. Open the preview release and expand **Assets**.
2. Download `Construct_<version>_x64-setup.exe` and `SHA256SUMS`.
3. In PowerShell, calculate the installer checksum:

   ```powershell
   (Get-FileHash .\Construct_<version>_x64-setup.exe -Algorithm SHA256).Hash.ToLower()
   ```

4. Compare the result with the installer line in `SHA256SUMS`.
5. Run the `.exe`. Node.js, Rust, Git, and the source repository are not
   required.

The separate `construct_<version>_x86_64-pc-windows-msvc.zip` contains the
standalone CLI, not the desktop installer. GitHub's automatically generated
“Source code” archives are not desktop or CLI downloads.

Because the Windows preview is not yet code-signed, Microsoft Defender
SmartScreen may display **Windows protected your PC**. After verifying that the
file came from the Construct release and its checksum matches, choose
**More info → Run anyway**. Do not disable SmartScreen globally.

The exact artifact names for every platform and the maintainer verification
process are documented in the [release guide](releasing.md).

## First five minutes

### 1. Add a Location

A **Location** is a local folder Construct is allowed to read and edit. It can
be a repository root or a narrower folder containing agent-generated Markdown.

1. Use the add-folder button in the **Locations** header.
2. Choose a folder in the native picker.
3. Select the new Location in the sidebar.

Construct discovers `.md` and `.markdown` files recursively. Common dependency,
cache, version-control, and build directories are excluded. Directory symlinks
are not followed.

Removing a Location from Construct removes its derived local index but never
deletes the folder or its Markdown files.

### 2. Open and arrange documents

Select a file under **Files** to open it in the active pane. You can:

- open multiple files as tabs;
- split the workspace vertically or horizontally;
- resize panes;
- move between open documents without losing their active mode;
- reveal a file in Finder;
- restore Locations, tabs, panes, and layout on the next launch.

Use `⌘P` to quick-open a file by partial name or relative path across registered
Locations. Use `↑` and `↓` to move through the results, `Enter` to open one, and
`Esc` to close the finder.

### 3. Choose a document mode

Every tab shares one in-memory buffer across its modes:

- **Preview** renders GitHub-flavored Markdown, local images, syntax
  highlighting, and Mermaid diagrams.
- **Edit** provides rich, Notion-like editing for the Markdown body.
- **Review** lets you select rendered text and leave comments for an agent.
- **Source** exposes the complete raw Markdown in CodeMirror.
- **Diff** compares the current document with Git `HEAD` when the file belongs
  to a Git worktree.

Git integration is strictly read-only. Construct never stages, commits,
checks out, resets, or merges.

## Edit without losing file fidelity

Construct uses explicit saves. Editing in **Edit**, **Review**, or **Source**
marks the tab as changed, but switching modes does not save it. Press `⌘S` or
use **Save** when you are ready.

Important behavior:

- **Edit** receives only the Markdown body. Existing YAML frontmatter is kept
  byte-for-byte and reattached.
- Opening **Edit** without changing anything does not normalize the file or
  create a false diff.
- Malformed or unclosed frontmatter prevents rich editing and keeps **Source**
  available as the safe fallback.
- Preview renders the unsaved local buffer, so you can inspect changes before
  saving.
- If another process changes a clean open file, Construct reloads it.
- If another process changes a file while your tab has unsaved edits,
  Construct enters an explicit conflict state instead of overwriting either
  version silently.

Construct does not autosave.

## Review a document with an agent

Review mode creates a portable feedback loop:

1. open **Review**;
2. select text in the rendered document;
3. describe what should change;
4. add the comment;
5. save the document;
6. choose **Copy for agent** and paste the self-contained handoff into a coding
   agent session.

Comments live in a versioned `construct-review:v1` HTML comment inside the
Markdown. Preview hides the block, Source keeps it inspectable, and agents can
read it without needing Construct.

Removing one comment preserves the others. Removing the last comment deletes
the review block and restores the original document content around it. Review
actions follow the same explicit-save behavior as normal editing.

## Search local knowledge

Press `⌘⇧F` or choose **Search** in the Files header to open knowledge search.
Unlike `⌘P`, it searches saved content and metadata rather than only filenames.

Search covers:

- Markdown body and headings;
- title, description, type, tags, and relative path;
- other indexable frontmatter values;
- one, several, or all selected Locations.

Each Location has a physically separate derived index. Cross-Location search
fans out to the chosen indexes and combines their local rankings; documents and
relations are not placed in one global database.

You can filter by Location, type, tag, path, technical role, findings, and
supported OKF lifecycle fields. Select results to:

- open the document;
- inspect direct outgoing links and backlinks;
- copy lightweight references without document bodies;
- copy a bounded context pack containing explicit selected documents.

Context packs preserve document boundaries and provenance. The budget first
reserves useful content for as many selected documents as possible, then
distributes the remaining characters proportionally. Large files cannot
silently consume the whole pack.

Recent submitted searches are stored only on this device, are limited to 20,
and can be cleared or disabled from the Search screen.

The index contains saved files only. Unsaved editor buffers do not appear in
knowledge search until you save them.

## Explore an OKF bundle

Construct recognizes Open Knowledge Format v0.1 and v0.2 bundles and consumes
future or partially conforming metadata tolerantly. It never imposes a closed
type taxonomy or rewrites OKF files automatically.

For a selected OKF Location, choose **Explore**:

- **List** browses concepts by type and tag.
- **Graph** visualizes bounded internal Markdown relationships.
- **Health** reports parser and conformance findings from the same native OKF
  implementation used by the CLI.

In **Health**:

- **Repository policy** applies the repository's `.constructignore`;
- **All Markdown** performs a strict inspection;
- **Run lint** refreshes findings from saved files;
- selecting a finding opens the affected document in Source;
- the agent handoff action copies a bounded repair prompt.

Health never fixes, saves, normalizes, or regenerates files. For terminal and CI
usage, see [CLI and OKF lint](cli.md).

## History and external changes

**History** combines the latest observed state of files across all registered
Locations. It retains one entry per file identity for 30 days and records
created, changed, renamed, and removed events without storing historical file
contents.

Select an existing history item to navigate to it. Removed files remain visible
until their entry expires. **Clear history** removes only Construct's event
metadata, not files.

## Connect a coding agent

On macOS, Windows, and Unix:

1. select a Location;
2. use the clipboard button in the **Locations** header;
3. paste the generated stdio configuration into your MCP client;
4. restart or reload the client if it does not discover newly added servers.

The generated server can read only the selected Location. It exposes overview,
recent activity, search, document reading, direct relationships, context-pack
assembly, and index status. It cannot edit files or run Git, shell, or database
queries.

See the [local MCP guide](mcp.md) for setup, manual allowlists, tools, and trust
boundaries.

## Keyboard shortcuts

The implemented macOS workspace shortcuts are:

| Action | Shortcut |
| --- | --- |
| Quick-open file | `⌘P` |
| Search knowledge | `⌘⇧F` |
| Save active document | `⌘S` |
| Close active tab | `⌘W` |
| Find inside the active editor | `⌘F` |
| Submit a Review comment | `⌘Enter` |

The finder and Search also support `Esc`; quick open supports `↑`, `↓`, and
`Enter`.

## Local data and privacy

Construct keeps application data in the operating system's application-data
directory. On macOS the default is:

```text
~/Library/Application Support/com.luisnovo.construct
```

It includes:

- `workspace.json` for Locations, layout, open tabs, history metadata, and
  optional recent queries;
- disposable per-Location indexes containing saved Markdown needed for local
  retrieval;
- the user-only local service token, local IPC endpoint, and derived activity cache;
- bounded diagnostic logs under `logs/`.

The diagnostic logs are local JSONL files:

- `construct.log` records desktop lifecycle plus local-service connection and
  launch events;
- `knowledge-service.log` records index reconciliation, full-text search,
  sanitized failures, and lexical fallback recovery;
- each file is capped at 1 MiB and retains two older generations.

On Windows, open the folder from PowerShell with:

```powershell
explorer "$env:APPDATA\com.luisnovo.construct\logs"
```

On macOS:

```text
~/Library/Application Support/com.luisnovo.construct/logs
```

Logs omit document content, filenames, repository paths, frontmatter values,
and search text. They may be deleted at any time while Construct is closed.

The registered Markdown files remain in their original folders and remain the
source of truth. Removing or rebuilding an index does not change those files.

Construct does not send document content, paths, search queries, or index
metrics to a remote service. An external MCP client controls what happens to
content after it requests it from Construct, so configure clients and model
providers according to your own privacy requirements.

## Troubleshooting

### The workspace stays on “Preparing your workspace”

Current startup restores the interactive workspace before full Location
reconciliation. If an old build appears stuck, launch the latest `main` build
from a terminal and inspect its output. A damaged derived index should not
block direct file navigation; rebuild the affected Location after the workspace
opens.

### Search is empty or an index is degraded

Wait for the Location status indicator to become ready. If it remains degraded
or failed, use **Rebuild index** for that Location. Rebuild deletes only
Construct's disposable cache and rereads saved Markdown.

If Search still fails, reproduce it once, close Construct, and share the files
from the local `logs/` directory with the maintainer. The
`knowledge-service.log` file distinguishes a valid empty result from a
full-text error, missing index schema, analyzer failure, or successful local
fallback without including the searched text or repository contents.

### Edit is unavailable

Open **Source** and inspect the YAML frontmatter. An opening `---` without a
matching closing delimiter, invalid UTF-8, or another unsafe document boundary
keeps rich Edit disabled to prevent data loss.

### An external edit conflicts with my changes

Do not reload until you decide which version to preserve. Keep your local
buffer to save it deliberately, or reload the external version after confirming
that discarding your unsaved edits is acceptable.

### An MCP client cannot see Construct

Confirm that:

- the Location is registered and indexed in the same Construct profile;
- the command path in the MCP configuration still points to an existing
  executable;
- the configuration contains `--allow <location-id>` or an intentional
  `--allow-all`;
- the client was restarted or reloaded after configuration;
- the installed Construct version supports local IPC on your operating system.

### A preview installer is blocked by the operating system

Do not disable platform security globally. Verify the artifact and checksum,
then decide whether to trust the explicitly labeled preview. On Windows, follow
the [tagged preview instructions](#install-a-tagged-preview) to use SmartScreen's
per-file **More info → Run anyway** path. Build from the reviewed source if you
do not want to run an unsigned preview. Trusted signing and notarization are not
available yet.

## More help

- [CLI and OKF lint](cli.md)
- [Local MCP access](mcp.md)
- [Security policy](../SECURITY.md)
- [Report an issue](https://github.com/lfnovo/construct/issues)
