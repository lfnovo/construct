# Security policy

Construct reads and edits files from user-selected local folders. Bugs involving filesystem boundaries or document rendering can therefore be security-sensitive.

## Supported versions

Until the first stable release, security fixes are applied only to the latest revision of `main`.

## Reporting a vulnerability

Use GitHub private vulnerability reporting when the repository becomes public. If that is unavailable, contact the maintainer privately through the GitHub profile rather than opening a public issue.

Include:

- the affected revision and macOS version;
- a minimal reproduction using synthetic files;
- the expected and observed security boundary;
- whether file contents, paths, or code execution may be exposed.

Do not include real credentials, private repositories, personal documents, or sensitive filesystem paths.

## Security boundaries

Construct is expected to:

- access content only under registered locations;
- keep Git integration read-only;
- sanitize rendered HTML;
- avoid following directory symlinks during discovery;
- require explicit user action for writes and external links;
- keep document contents out of workspace history and telemetry.

Signing, notarization, and automatic updates are not yet available. Builds produced directly from source should be treated as development previews.
