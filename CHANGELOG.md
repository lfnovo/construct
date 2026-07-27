# Changelog

All notable changes to Construct will be documented in this file.

The project follows [Semantic Versioning](https://semver.org/) once public releases begin.

## [Unreleased]

### Added

- Local-first macOS workspace for recursively discovered Markdown files.
- Tabs, split panes, editable source, rendered preview, Mermaid, and local images.
- Rich Markdown editing with contextual formatting and lossless YAML frontmatter preservation.
- Persistent document review comments with agent-ready clipboard handoff and exact cleanup.
- Filesystem monitoring with deduplicated recent history.
- Read-only Git status and diff.
- Open Knowledge Format detection, metadata inspection, navigation, filtering, and graph exploration.
- Stateless `construct okf lint` validation with deterministic text/JSON output and CI exit codes.
- Light and dark themes, keyboard shortcuts, Finder integration, and workspace restoration.

### Changed

- Renamed the application from Agent Context to Construct.

### Security

- Markdown preview is sanitized and file access is limited to user-selected locations.
