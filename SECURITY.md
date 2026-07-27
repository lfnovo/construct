# Security policy

Construct reads and edits files from user-selected local folders and can expose
explicitly allowed Locations to local MCP clients. Bugs involving filesystem
boundaries, document rendering, index isolation, or agent access can therefore
be security-sensitive.

## Supported versions

Until the first stable release, security fixes are applied only to the latest revision of `main`.

## Reporting a vulnerability

Use GitHub private vulnerability reporting when the repository becomes public. If that is unavailable, contact the maintainer privately through the GitHub profile rather than opening a public issue.

Include:

- the affected revision, Construct version, and operating system;
- a minimal reproduction using synthetic files;
- the expected and observed security boundary;
- whether file contents, paths, agent access, or code execution may be exposed.

Do not include real credentials, private repositories, personal documents, or sensitive filesystem paths.

## Security boundaries

Construct is expected to:

- access content only under registered locations;
- keep Git integration read-only;
- sanitize rendered HTML;
- avoid following directory symlinks during discovery;
- require explicit user action for writes and external links;
- keep document contents out of workspace history and telemetry;
- keep per-Location retrieval indexes physically isolated;
- authenticate the local knowledge-service transport with user-only local
  state;
- require explicit MCP Location allowlists;
- expose no source mutation, shell, Git write, arbitrary SQL, or arbitrary
  filesystem read through MCP;
- make no outbound request from the core application or MCP server.

The initial macOS release workflow uses ad-hoc signing only. It does not
establish developer identity or provide notarization. Windows artifacts are not
yet code-signed, and automatic updates are not available. Treat source builds
and release candidates as development previews. Trusted distribution work is
tracked in [issue #19](https://github.com/lfnovo/construct/issues/19).

An MCP client controls what happens to content after requesting it from
Construct. Configure the client's model provider and retention policy according
to your own privacy requirements.
