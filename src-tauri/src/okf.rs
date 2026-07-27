use pulldown_cmark::{Event, Options, Parser, Tag};
use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value as YamlValue};
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Component, Path, PathBuf},
};

const MAX_DOCUMENT_BYTES: usize = 8 * 1024 * 1024;
const MAX_FRONTMATTER_BYTES: usize = 1024 * 1024;
const MAX_YAML_DEPTH: usize = 64;
const REVIEW_MARKER: &str = "<!-- construct-review:v1";

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub(crate) enum OkfValue {
    Null,
    Boolean { value: bool },
    Integer { value: String },
    UnsignedInteger { value: String },
    Float { value: String },
    String { value: String },
    Sequence { items: Vec<OkfValue> },
    Mapping { entries: Vec<OkfMappingEntry> },
    Tagged { tag: String, value: Box<OkfValue> },
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OkfMappingEntry {
    key: OkfValue,
    value: OkfValue,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OkfNamedValue {
    name: String,
    value: OkfValue,
}

#[derive(Clone, Debug, Default, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OkfMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    r#type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resource: Option<String>,
    tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    effective_timestamp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    okf_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stale_after: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sources: Option<OkfValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generated: Option<OkfValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    verified: Option<OkfValue>,
    extra: Vec<OkfNamedValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    raw: Option<OkfValue>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SourceRange {
    pub(crate) start_line: usize,
    pub(crate) start_column: usize,
    pub(crate) end_line: usize,
    pub(crate) end_column: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OkfFinding {
    pub(crate) code: String,
    pub(crate) severity: FindingSeverity,
    pub(crate) message: String,
    pub(crate) relative_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) range: Option<SourceRange>,
}

#[derive(Clone, Copy, Debug, Eq, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum FindingSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DocumentKind {
    Concept,
    Index,
    Log,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum LinkOrigin {
    Markdown,
    Metadata,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) enum LinkStatus {
    Candidate,
    Resolved,
    Unresolved,
    External,
    Fragment,
    OutsideBundle,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OkfLink {
    target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    fragment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved_path: Option<String>,
    origin: LinkOrigin,
    #[serde(skip_serializing_if = "Option::is_none")]
    field: Option<String>,
    status: LinkStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    range: Option<SourceRange>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OkfInspection {
    kind: DocumentKind,
    relative_path: String,
    has_frontmatter: bool,
    metadata: OkfMetadata,
    links: Vec<OkfLink>,
    findings: Vec<OkfFinding>,
    is_conformant: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct IndexableLink {
    pub(crate) target: String,
    pub(crate) target_relative_path: Option<String>,
    pub(crate) fragment: Option<String>,
    pub(crate) origin: String,
    pub(crate) field: Option<String>,
    pub(crate) status: String,
    pub(crate) start_line: Option<usize>,
    pub(crate) end_line: Option<usize>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OkfConcept {
    id: String,
    path: String,
    relative_path: String,
    r#type: String,
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<String>,
    outgoing_paths: Vec<String>,
    incoming_paths: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct OkfBundleSnapshot {
    detected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) declared_version: Option<String>,
    pub(crate) document_count: usize,
    pub(crate) finding_count: usize,
    pub(crate) findings: Vec<OkfFinding>,
    pub(crate) concepts: Vec<OkfConcept>,
    pub(crate) ignored_paths: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InspectDocumentRequest {
    content: String,
    relative_path: String,
    source_path: String,
    bundle_root: String,
    #[serde(default)]
    is_bundle_root: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct BundleFile {
    pub(crate) path: PathBuf,
    pub(crate) relative_path: String,
}

impl OkfBundleSnapshot {
    pub(crate) fn apply_conformance_policy(
        &mut self,
        ignored_paths: Vec<String>,
        retain_ignored_findings: bool,
    ) {
        let ignored = ignored_paths.iter().cloned().collect::<HashSet<_>>();
        self.concepts
            .retain(|concept| !ignored.contains(&concept.relative_path));
        if !retain_ignored_findings {
            self.findings
                .retain(|finding| !ignored.contains(&finding.relative_path));
        }
        self.finding_count = self
            .findings
            .iter()
            .filter(|finding| !ignored.contains(&finding.relative_path))
            .count();
        self.ignored_paths = ignored_paths;
    }
}

#[derive(Debug)]
struct Frontmatter<'a> {
    has_frontmatter: bool,
    source: Option<&'a str>,
    body: &'a str,
    body_offset: usize,
    error: Option<String>,
}

fn finding(
    code: &str,
    severity: FindingSeverity,
    message: impl Into<String>,
    relative_path: &str,
    range: Option<SourceRange>,
) -> OkfFinding {
    OkfFinding {
        code: code.to_string(),
        severity,
        message: message.into(),
        relative_path: relative_path.to_string(),
        range,
    }
}

fn document_kind(relative_path: &str) -> DocumentKind {
    match relative_path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "index.md" | "index.markdown" => DocumentKind::Index,
        "log.md" | "log.markdown" => DocumentKind::Log,
        _ => DocumentKind::Concept,
    }
}

fn split_frontmatter(content: &str) -> Frontmatter<'_> {
    let opening_length = if content.starts_with("---\r\n") {
        5
    } else if content.starts_with("---\n") {
        4
    } else {
        return Frontmatter {
            has_frontmatter: false,
            source: None,
            body: content,
            body_offset: 0,
            error: None,
        };
    };

    let bytes = content.as_bytes();
    let mut line_start = opening_length;
    while line_start <= bytes.len() {
        let line_end = content[line_start..]
            .find('\n')
            .map(|offset| line_start + offset + 1)
            .unwrap_or(bytes.len());
        let line = content[line_start..line_end]
            .trim_end_matches(['\r', '\n'])
            .trim_end_matches([' ', '\t']);
        if line == "---" {
            let source_end = line_start.saturating_sub(1);
            let source_end = if source_end > opening_length && bytes[source_end - 1] == b'\r' {
                source_end - 1
            } else {
                source_end
            };
            return Frontmatter {
                has_frontmatter: true,
                source: Some(&content[opening_length..source_end]),
                body: &content[line_end..],
                body_offset: line_end,
                error: None,
            };
        }
        if line_end == bytes.len() {
            break;
        }
        line_start = line_end;
    }

    Frontmatter {
        has_frontmatter: true,
        source: None,
        body: content,
        body_offset: 0,
        error: Some("The YAML frontmatter block is not closed.".to_string()),
    }
}

fn without_review_block(body: &str) -> (&str, usize) {
    if !body.starts_with(REVIEW_MARKER) {
        return (body, 0);
    }
    let Some(end) = body.find("-->") else {
        return (body, 0);
    };
    let mut body_start = end + 3;
    if body[body_start..].starts_with("\r\n") {
        body_start += 2;
    } else if body[body_start..].starts_with('\n') {
        body_start += 1;
    }
    (&body[body_start..], body_start)
}

pub(crate) fn visible_markdown_body(content: &str) -> &str {
    let frontmatter = split_frontmatter(content);
    without_review_block(frontmatter.body).0
}

fn source_range(content: &str, start: usize, end: usize) -> SourceRange {
    fn position(content: &str, offset: usize) -> (usize, usize) {
        let prefix = &content[..offset.min(content.len())];
        let line = prefix.bytes().filter(|byte| *byte == b'\n').count() + 1;
        let column = prefix
            .rsplit_once('\n')
            .map(|(_, tail)| tail.chars().count() + 1)
            .unwrap_or_else(|| prefix.chars().count() + 1);
        (line, column)
    }
    let (start_line, start_column) = position(content, start);
    let (end_line, end_column) = position(content, end);
    SourceRange {
        start_line,
        start_column,
        end_line,
        end_column,
    }
}

fn yaml_to_okf(value: &YamlValue, depth: usize) -> Result<OkfValue, String> {
    if depth > MAX_YAML_DEPTH {
        return Err(format!(
            "YAML nesting exceeds the supported depth of {MAX_YAML_DEPTH}."
        ));
    }
    Ok(match value {
        YamlValue::Null => OkfValue::Null,
        YamlValue::Bool(value) => OkfValue::Boolean { value: *value },
        YamlValue::Number(value) => {
            if let Some(value) = value.as_i64() {
                OkfValue::Integer {
                    value: value.to_string(),
                }
            } else if let Some(value) = value.as_u64() {
                OkfValue::UnsignedInteger {
                    value: value.to_string(),
                }
            } else {
                OkfValue::Float {
                    value: value
                        .as_f64()
                        .ok_or_else(|| "Cannot represent this YAML number.".to_string())?
                        .to_string(),
                }
            }
        }
        YamlValue::String(value) => OkfValue::String {
            value: value.clone(),
        },
        YamlValue::Sequence(values) => OkfValue::Sequence {
            items: values
                .iter()
                .map(|value| yaml_to_okf(value, depth + 1))
                .collect::<Result<Vec<_>, _>>()?,
        },
        YamlValue::Mapping(mapping) => OkfValue::Mapping {
            entries: mapping
                .iter()
                .map(|(key, value)| {
                    Ok(OkfMappingEntry {
                        key: yaml_to_okf(key, depth + 1)?,
                        value: yaml_to_okf(value, depth + 1)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?,
        },
        YamlValue::Tagged(tagged) => OkfValue::Tagged {
            tag: tagged.tag.to_string(),
            value: Box::new(yaml_to_okf(&tagged.value, depth + 1)?),
        },
    })
}

fn scalar_string(value: &YamlValue) -> Option<String> {
    match value {
        YamlValue::String(value) => Some(value.clone()),
        YamlValue::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn mapping_value<'a>(mapping: &'a Mapping, key: &str) -> Option<&'a YamlValue> {
    mapping.get(YamlValue::String(key.to_string()))
}

fn nested_scalar(mapping: &Mapping, field: &str, nested: &str) -> Option<String> {
    let YamlValue::Mapping(value) = mapping_value(mapping, field)? else {
        return None;
    };
    scalar_string(mapping_value(value, nested)?)
}

fn normalized_scalar(
    mapping: &Mapping,
    field: &str,
    relative_path: &str,
    findings: &mut Vec<OkfFinding>,
    allow_number: bool,
) -> Option<String> {
    let value = mapping_value(mapping, field)?;
    if let YamlValue::String(value) = value {
        return Some(value.clone());
    }
    if allow_number {
        if let YamlValue::Number(value) = value {
            return Some(value.to_string());
        }
    }
    findings.push(finding(
        &format!("OKF_{}_INVALID", field.to_ascii_uppercase()),
        FindingSeverity::Warning,
        format!("The {field} field should be a string."),
        relative_path,
        None,
    ));
    None
}

fn normalize_tags(
    mapping: &Mapping,
    relative_path: &str,
    findings: &mut Vec<OkfFinding>,
) -> Vec<String> {
    let Some(value) = mapping_value(mapping, "tags") else {
        return Vec::new();
    };
    match value {
        YamlValue::Sequence(values) => values
            .iter()
            .filter_map(|value| {
                if let YamlValue::String(value) = value {
                    Some(value.clone())
                } else {
                    findings.push(finding(
                        "OKF_TAG_ITEM_INVALID",
                        FindingSeverity::Warning,
                        "Every tags entry should be a string.",
                        relative_path,
                        None,
                    ));
                    None
                }
            })
            .collect(),
        YamlValue::String(value) => {
            findings.push(finding(
                "OKF_TAGS_SCALAR",
                FindingSeverity::Info,
                "A scalar tags value was accepted as a one-item list for compatibility.",
                relative_path,
                None,
            ));
            vec![value.clone()]
        }
        _ => {
            findings.push(finding(
                "OKF_TAGS_INVALID",
                FindingSeverity::Warning,
                "The tags field should be a list of strings.",
                relative_path,
                None,
            ));
            Vec::new()
        }
    }
}

fn normalize_metadata(
    mapping: &Mapping,
    relative_path: &str,
    findings: &mut Vec<OkfFinding>,
) -> Result<OkfMetadata, String> {
    let r#type = normalized_scalar(mapping, "type", relative_path, findings, false);
    let title = normalized_scalar(mapping, "title", relative_path, findings, false);
    let description = normalized_scalar(mapping, "description", relative_path, findings, false);
    let resource = normalized_scalar(mapping, "resource", relative_path, findings, false);
    let timestamp = normalized_scalar(mapping, "timestamp", relative_path, findings, true);
    let okf_version = normalized_scalar(mapping, "okf_version", relative_path, findings, true);
    let status = normalized_scalar(mapping, "status", relative_path, findings, false);
    let stale_after = normalized_scalar(mapping, "stale_after", relative_path, findings, true);
    let generated_at = nested_scalar(mapping, "generated", "at");

    if let Some(version) = okf_version.as_deref() {
        if version != "0.1" && version != "0.2" {
            findings.push(finding(
                "OKF_VERSION_UNSUPPORTED",
                FindingSeverity::Info,
                format!(
                    "OKF version {version} is newer or unknown; Construct is reading it in compatibility mode."
                ),
                relative_path,
                None,
            ));
        }
    }
    if let Some(value) = status.as_deref() {
        if !matches!(value, "draft" | "stable" | "deprecated") {
            findings.push(finding(
                "OKF_STATUS_UNKNOWN",
                FindingSeverity::Info,
                format!("The lifecycle status '{value}' is not defined by OKF v0.2."),
                relative_path,
                None,
            ));
        }
    }

    let known = [
        "type",
        "title",
        "description",
        "resource",
        "tags",
        "timestamp",
        "okf_version",
        "sources",
        "generated",
        "verified",
        "status",
        "stale_after",
    ];
    let mut extra = Vec::new();
    for (key, value) in mapping {
        match key {
            YamlValue::String(name) if !known.contains(&name.as_str()) => {
                extra.push(OkfNamedValue {
                    name: name.clone(),
                    value: yaml_to_okf(value, 1)?,
                });
            }
            YamlValue::String(_) => {}
            _ => findings.push(finding(
                "OKF_METADATA_KEY_INVALID",
                FindingSeverity::Warning,
                "Top-level frontmatter keys should be strings.",
                relative_path,
                None,
            )),
        }
    }

    let typed = |field: &str| {
        mapping_value(mapping, field)
            .map(|value| yaml_to_okf(value, 1))
            .transpose()
    };
    Ok(OkfMetadata {
        r#type,
        title,
        description,
        resource,
        tags: normalize_tags(mapping, relative_path, findings),
        effective_timestamp: generated_at.or_else(|| timestamp.clone()),
        timestamp,
        okf_version,
        status,
        stale_after,
        sources: typed("sources")?,
        generated: typed("generated")?,
        verified: typed("verified")?,
        extra,
        raw: Some(yaml_to_okf(&YamlValue::Mapping(mapping.clone()), 0)?),
    })
}

fn has_uri_scheme(target: &str) -> bool {
    let Some(colon) = target.find(':') else {
        return false;
    };
    let scheme = &target[..colon];
    !scheme.is_empty()
        && scheme.chars().enumerate().all(|(index, character)| {
            if index == 0 {
                character.is_ascii_alphabetic()
            } else {
                character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
            }
        })
}

fn percent_decode(value: &str) -> String {
    fn nibble(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) = (nibble(bytes[index + 1]), nibble(bytes[index + 2])) {
                decoded.push((high << 4) | low);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(decoded).unwrap_or_else(|_| value.to_string())
}

fn normalize_lexical(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Prefix(_) | Component::RootDir | Component::Normal(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

fn split_link_target(target: &str) -> (&str, Option<String>) {
    let (path, fragment) = target
        .split_once('#')
        .map(|(path, fragment)| (path, Some(fragment.to_string())))
        .unwrap_or((target, None));
    let path = path.split_once('?').map(|(path, _)| path).unwrap_or(path);
    (path, fragment)
}

fn resolve_link(
    target: &str,
    source_path: &Path,
    bundle_root: &Path,
    origin: LinkOrigin,
    field: Option<String>,
    range: Option<SourceRange>,
) -> OkfLink {
    let bundle_root = normalize_lexical(bundle_root);
    let (path_part, fragment) = split_link_target(target);
    if path_part.is_empty() {
        return OkfLink {
            target: target.to_string(),
            fragment,
            resolved_path: Some(source_path.to_string_lossy().to_string()),
            origin,
            field,
            status: LinkStatus::Fragment,
            range,
        };
    }
    if has_uri_scheme(path_part) || path_part.starts_with("//") {
        return OkfLink {
            target: target.to_string(),
            fragment,
            resolved_path: None,
            origin,
            field,
            status: LinkStatus::External,
            range,
        };
    }

    let decoded = percent_decode(path_part);
    let candidate = if decoded.starts_with('/') {
        bundle_root.join(decoded.trim_start_matches('/'))
    } else {
        source_path
            .parent()
            .unwrap_or(&bundle_root)
            .join(decoded.as_str())
    };
    let normalized = normalize_lexical(&candidate);
    let inside = normalized.starts_with(&bundle_root);
    OkfLink {
        target: target.to_string(),
        fragment,
        resolved_path: inside.then(|| normalized.to_string_lossy().to_string()),
        origin,
        field,
        status: if inside {
            LinkStatus::Candidate
        } else {
            LinkStatus::OutsideBundle
        },
        range,
    }
}

fn markdown_links(
    body: &str,
    body_offset: usize,
    content: &str,
    source_path: &Path,
    bundle_root: &Path,
) -> Vec<OkfLink> {
    let mut links = Vec::new();
    let mut options = Options::empty();
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TABLES);
    for (event, range) in Parser::new_ext(body, options).into_offset_iter() {
        if let Event::Start(Tag::Link { dest_url, .. }) = event {
            links.push(resolve_link(
                dest_url.as_ref(),
                source_path,
                bundle_root,
                LinkOrigin::Markdown,
                None,
                Some(source_range(
                    content,
                    body_offset + range.start,
                    body_offset + range.end,
                )),
            ));
        }
    }
    links
}

fn metadata_path_values(mapping: &Mapping) -> Vec<(String, String)> {
    fn push_scalar(output: &mut Vec<(String, String)>, field: &str, value: Option<&YamlValue>) {
        if let Some(value) = value.and_then(scalar_string) {
            output.push((field.to_string(), value));
        }
    }
    let mut output = Vec::new();
    push_scalar(&mut output, "resource", mapping_value(mapping, "resource"));
    push_scalar(
        &mut output,
        "computation",
        mapping_value(mapping, "computation"),
    );
    for parent in ["executor", "attester"] {
        if let Some(YamlValue::Mapping(value)) = mapping_value(mapping, parent) {
            push_scalar(
                &mut output,
                &format!("{parent}.resource"),
                mapping_value(value, "resource"),
            );
        }
    }
    if let Some(YamlValue::Sequence(sources)) = mapping_value(mapping, "sources") {
        for (index, source) in sources.iter().enumerate() {
            if let YamlValue::Mapping(source) = source {
                push_scalar(
                    &mut output,
                    &format!("sources[{index}].resource"),
                    mapping_value(source, "resource"),
                );
            }
        }
    }
    output
}

fn inspect(
    content: &str,
    relative_path: &str,
    source_path: &Path,
    bundle_root: &Path,
    is_bundle_root: bool,
) -> OkfInspection {
    let kind = document_kind(relative_path);
    if content.len() > MAX_DOCUMENT_BYTES {
        let findings = vec![finding(
            "OKF_DOCUMENT_TOO_LARGE",
            FindingSeverity::Error,
            format!(
                "This document exceeds the supported inspection limit of {} MiB.",
                MAX_DOCUMENT_BYTES / 1024 / 1024
            ),
            relative_path,
            None,
        )];
        return OkfInspection {
            kind,
            relative_path: relative_path.to_string(),
            has_frontmatter: content.starts_with("---\n") || content.starts_with("---\r\n"),
            metadata: OkfMetadata::default(),
            links: Vec::new(),
            findings,
            is_conformant: false,
        };
    }
    let mut findings = Vec::new();
    let frontmatter = split_frontmatter(content);
    let mut metadata = OkfMetadata::default();
    let mut mapping: Option<Mapping> = None;

    if let Some(error) = frontmatter.error.as_deref() {
        findings.push(finding(
            "OKF_FRONTMATTER_UNCLOSED",
            FindingSeverity::Error,
            error,
            relative_path,
            None,
        ));
    } else if let Some(source) = frontmatter.source {
        if source.len() > MAX_FRONTMATTER_BYTES {
            findings.push(finding(
                "OKF_FRONTMATTER_TOO_LARGE",
                FindingSeverity::Error,
                format!(
                    "The YAML frontmatter exceeds the supported limit of {} MiB.",
                    MAX_FRONTMATTER_BYTES / 1024 / 1024
                ),
                relative_path,
                None,
            ));
        } else {
            match serde_yaml::from_str::<YamlValue>(source) {
                Ok(YamlValue::Mapping(value)) => {
                    match normalize_metadata(&value, relative_path, &mut findings) {
                        Ok(value) => metadata = value,
                        Err(error) => findings.push(finding(
                            "OKF_YAML_DEPTH_EXCEEDED",
                            FindingSeverity::Error,
                            error,
                            relative_path,
                            None,
                        )),
                    }
                    mapping = Some(value);
                }
                Ok(YamlValue::Null) => {
                    mapping = Some(Mapping::new());
                    metadata.raw = Some(OkfValue::Null);
                }
                Ok(_) => findings.push(finding(
                    "OKF_FRONTMATTER_NOT_MAPPING",
                    FindingSeverity::Error,
                    "YAML frontmatter should be a mapping of field names to values.",
                    relative_path,
                    None,
                )),
                Err(error) => {
                    let range = error.location().map(|location| SourceRange {
                        start_line: location.line() + 1,
                        start_column: location.column(),
                        end_line: location.line() + 1,
                        end_column: location.column() + 1,
                    });
                    findings.push(finding(
                        "OKF_YAML_INVALID",
                        FindingSeverity::Error,
                        format!("The YAML frontmatter is invalid: {error}"),
                        relative_path,
                        range,
                    ));
                }
            }
        }
    }

    match kind {
        DocumentKind::Concept => {
            if !frontmatter.has_frontmatter {
                findings.push(finding(
                    "OKF_FRONTMATTER_REQUIRED",
                    FindingSeverity::Error,
                    "Concept documents need YAML frontmatter.",
                    relative_path,
                    None,
                ));
            } else if metadata
                .r#type
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
                .is_empty()
            {
                findings.push(finding(
                    "OKF_TYPE_REQUIRED",
                    FindingSeverity::Error,
                    "The required type field is missing.",
                    relative_path,
                    None,
                ));
            }
        }
        DocumentKind::Log => {
            if frontmatter.has_frontmatter {
                findings.push(finding(
                    "OKF_LOG_FRONTMATTER",
                    FindingSeverity::Warning,
                    "log.md normally has no frontmatter.",
                    relative_path,
                    None,
                ));
            }
            let has_iso_heading = content.lines().any(|line| {
                let value = line.trim().strip_prefix("## ").unwrap_or_default();
                value.len() == 10
                    && value.as_bytes().get(4) == Some(&b'-')
                    && value.as_bytes().get(7) == Some(&b'-')
                    && value.chars().enumerate().all(|(index, character)| {
                        matches!(index, 4 | 7) || character.is_ascii_digit()
                    })
            });
            if !has_iso_heading {
                findings.push(finding(
                    "OKF_LOG_DATE_HEADING_REQUIRED",
                    FindingSeverity::Warning,
                    "Use ISO date headings (YYYY-MM-DD) for log entries.",
                    relative_path,
                    None,
                ));
            }
        }
        DocumentKind::Index => {
            if frontmatter.has_frontmatter {
                let allowed = is_bundle_root
                    && metadata.okf_version.is_some()
                    && mapping
                        .as_ref()
                        .map(|value| {
                            value.keys().all(|key| {
                                matches!(key, YamlValue::String(name) if name == "okf_version")
                            })
                        })
                        .unwrap_or(false);
                if !allowed {
                    findings.push(finding(
                        "OKF_INDEX_FRONTMATTER",
                        FindingSeverity::Warning,
                        "index.md normally has no frontmatter; a root index may declare only okf_version.",
                        relative_path,
                        None,
                    ));
                }
            }
        }
    }

    let (body, review_offset) = without_review_block(frontmatter.body);
    let mut links = markdown_links(
        body,
        frontmatter.body_offset + review_offset,
        content,
        source_path,
        bundle_root,
    );
    if let Some(mapping) = mapping.as_ref() {
        links.extend(
            metadata_path_values(mapping)
                .into_iter()
                .map(|(field, target)| {
                    resolve_link(
                        &target,
                        source_path,
                        bundle_root,
                        LinkOrigin::Metadata,
                        Some(field),
                        None,
                    )
                }),
        );
    }
    for link in &links {
        if link.status == LinkStatus::OutsideBundle {
            findings.push(finding(
                "OKF_LINK_OUTSIDE_BUNDLE",
                FindingSeverity::Warning,
                format!("The link '{}' escapes the bundle root.", link.target),
                relative_path,
                link.range.clone(),
            ));
        }
    }

    let is_conformant = findings
        .iter()
        .all(|finding| finding.severity != FindingSeverity::Error);
    OkfInspection {
        kind,
        relative_path: relative_path.to_string(),
        has_frontmatter: frontmatter.has_frontmatter,
        metadata,
        links,
        findings,
        is_conformant,
    }
}

pub(crate) fn inspect_document(request: InspectDocumentRequest) -> OkfInspection {
    inspect(
        &request.content,
        &request.relative_path,
        Path::new(&request.source_path),
        Path::new(&request.bundle_root),
        request.is_bundle_root,
    )
}

pub(crate) fn inspect_saved_document(
    content: &str,
    relative_path: &str,
    source_path: &Path,
    bundle_root: &Path,
    is_bundle_root: bool,
) -> OkfInspection {
    inspect(
        content,
        relative_path,
        source_path,
        bundle_root,
        is_bundle_root,
    )
}

pub(crate) fn indexable_links(
    inspection: &OkfInspection,
    bundle_root: &Path,
) -> Vec<IndexableLink> {
    inspection
        .links
        .iter()
        .map(|link| {
            let target_relative_path = link
                .resolved_path
                .as_deref()
                .and_then(|path| Path::new(path).strip_prefix(bundle_root).ok())
                .map(|path| path.to_string_lossy().replace('\\', "/"));
            IndexableLink {
                target: link.target.clone(),
                target_relative_path,
                fragment: link.fragment.clone(),
                origin: match link.origin {
                    LinkOrigin::Markdown => "markdown",
                    LinkOrigin::Metadata => "metadata",
                }
                .to_string(),
                field: link.field.clone(),
                status: match link.status {
                    LinkStatus::Candidate => "candidate",
                    LinkStatus::Resolved => "resolved",
                    LinkStatus::Unresolved => "unresolved",
                    LinkStatus::External => "external",
                    LinkStatus::Fragment => "fragment",
                    LinkStatus::OutsideBundle => "outsideBundle",
                }
                .to_string(),
                start_line: link.range.as_ref().map(|range| range.start_line),
                end_line: link.range.as_ref().map(|range| range.end_line),
            }
        })
        .collect()
}

fn is_markdown_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| matches!(extension.to_ascii_lowercase().as_str(), "md" | "markdown"))
        .unwrap_or(false)
}

pub(crate) fn inspect_bundle(
    root: &Path,
    files: Vec<BundleFile>,
) -> Result<OkfBundleSnapshot, String> {
    let mut documents = Vec::new();
    let mut read_findings = Vec::new();
    for file in &files {
        let content = match fs::read_to_string(&file.path) {
            Ok(content) => content,
            Err(error) => {
                read_findings.push(finding(
                    "OKF_FILE_UNREADABLE",
                    FindingSeverity::Error,
                    format!("Could not read this Markdown file: {error}"),
                    &file.relative_path,
                    None,
                ));
                continue;
            }
        };
        documents.push((
            file,
            inspect(
                &content,
                &file.relative_path,
                &file.path,
                root,
                file.relative_path.eq_ignore_ascii_case("index.md"),
            ),
        ));
    }

    let all_paths = files
        .iter()
        .map(|file| normalize_lexical(&file.path))
        .collect::<HashSet<_>>();
    for (_, inspection) in &mut documents {
        let mut broken = Vec::new();
        for link in &mut inspection.links {
            if link.status != LinkStatus::Candidate {
                continue;
            }
            let Some(path) = link.resolved_path.as_deref().map(PathBuf::from) else {
                continue;
            };
            if all_paths.contains(&path) {
                link.status = LinkStatus::Resolved;
            } else {
                link.status = LinkStatus::Unresolved;
                if is_markdown_path(&path) {
                    broken.push((
                        link.target.clone(),
                        link.range.clone(),
                        inspection.relative_path.clone(),
                    ));
                }
            }
        }
        for (target, range, relative_path) in broken {
            inspection.findings.push(finding(
                "OKF_LINK_BROKEN",
                FindingSeverity::Warning,
                format!("The internal link '{target}' does not resolve to a file in this bundle."),
                &relative_path,
                range,
            ));
        }
    }

    let root_inspection = documents
        .iter()
        .find(|(file, _)| file.relative_path.eq_ignore_ascii_case("index.md"))
        .map(|(_, inspection)| inspection);
    let declared_version =
        root_inspection.and_then(|inspection| inspection.metadata.okf_version.clone());
    let has_typed_concept = documents.iter().any(|(_, inspection)| {
        inspection.kind == DocumentKind::Concept
            && inspection
                .metadata
                .r#type
                .as_deref()
                .map(str::trim)
                .is_some_and(|value| !value.is_empty())
    });
    // A version declaration is sufficient on its own. For undeclared bundles,
    // require both the OKF structural signal (a root index) and typed concepts.
    // This avoids classifying ordinary repositories as OKF merely because they
    // contain examples or test fixtures with a `type` field.
    let detected = declared_version.is_some() || (root_inspection.is_some() && has_typed_concept);

    let concept_paths = documents
        .iter()
        .filter(|(_, inspection)| inspection.kind == DocumentKind::Concept)
        .map(|(file, _)| normalize_lexical(&file.path))
        .collect::<HashSet<_>>();
    let mut incoming: HashMap<PathBuf, Vec<String>> = HashMap::new();
    let mut outgoing_by_source: HashMap<PathBuf, Vec<String>> = HashMap::new();
    for (file, inspection) in &documents {
        if inspection.kind != DocumentKind::Concept {
            continue;
        }
        let source = normalize_lexical(&file.path);
        let mut outgoing = Vec::new();
        for target in inspection.links.iter().filter_map(|link| {
            (link.status == LinkStatus::Resolved)
                .then_some(link.resolved_path.as_deref())
                .flatten()
                .map(PathBuf::from)
        }) {
            if concept_paths.contains(&target) && !outgoing.iter().any(|path| path == &target) {
                incoming
                    .entry(target.clone())
                    .or_default()
                    .push(source.to_string_lossy().to_string());
                outgoing.push(target);
            }
        }
        outgoing_by_source.insert(
            source,
            outgoing
                .into_iter()
                .map(|path| path.to_string_lossy().to_string())
                .collect(),
        );
    }

    let mut concepts = documents
        .iter()
        .filter(|(_, inspection)| inspection.kind == DocumentKind::Concept)
        .map(|(file, inspection)| {
            let path = normalize_lexical(&file.path);
            let relative_path = file.relative_path.clone();
            let filename = Path::new(&relative_path)
                .file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or(&relative_path)
                .to_string();
            OkfConcept {
                id: relative_path
                    .strip_suffix(".markdown")
                    .or_else(|| relative_path.strip_suffix(".md"))
                    .unwrap_or(&relative_path)
                    .to_string(),
                path: path.to_string_lossy().to_string(),
                relative_path,
                r#type: inspection
                    .metadata
                    .r#type
                    .clone()
                    .unwrap_or_else(|| "Unclassified".to_string()),
                title: inspection.metadata.title.clone().unwrap_or(filename),
                description: inspection.metadata.description.clone(),
                tags: inspection.metadata.tags.clone(),
                timestamp: inspection.metadata.effective_timestamp.clone(),
                outgoing_paths: outgoing_by_source.remove(&path).unwrap_or_default(),
                incoming_paths: incoming.remove(&path).unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>();
    concepts.sort_by(|left, right| {
        left.relative_path
            .to_ascii_lowercase()
            .cmp(&right.relative_path.to_ascii_lowercase())
    });
    let mut findings = documents
        .iter()
        .flat_map(|(_, inspection)| inspection.findings.clone())
        .collect::<Vec<_>>();
    findings.extend(read_findings);
    let finding_count = findings.len();
    Ok(OkfBundleSnapshot {
        detected,
        declared_version,
        document_count: files.len(),
        finding_count,
        findings,
        concepts,
        ignored_paths: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FixtureCase {
        bundle: String,
        relative_path: String,
        kind: String,
        conformant: bool,
        r#type: Option<String>,
        okf_version: Option<String>,
        finding_codes: Vec<String>,
    }

    fn fixture_root(case: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/okf")
            .join(case)
    }

    fn inspect_fixture(case: &str, relative_path: &str) -> OkfInspection {
        let root = fixture_root(case);
        let path = root.join(relative_path);
        let content = fs::read_to_string(&path).expect("read fixture");
        inspect(
            &content,
            relative_path,
            &path,
            &root,
            relative_path == "index.md",
        )
    }

    #[test]
    fn compatibility_manifest_is_the_extensible_document_contract() {
        let manifest = fixture_root("").join("cases.json");
        let cases: Vec<FixtureCase> =
            serde_json::from_str(&fs::read_to_string(manifest).expect("read fixture manifest"))
                .expect("parse fixture manifest");
        assert!(!cases.is_empty());
        for case in cases {
            let inspection = inspect_fixture(&case.bundle, &case.relative_path);
            let actual_kind = match inspection.kind {
                DocumentKind::Concept => "concept",
                DocumentKind::Index => "index",
                DocumentKind::Log => "log",
            };
            assert_eq!(actual_kind, case.kind, "{}", case.relative_path);
            assert_eq!(
                inspection.is_conformant, case.conformant,
                "{}",
                case.relative_path
            );
            assert_eq!(
                inspection.metadata.r#type, case.r#type,
                "{}",
                case.relative_path
            );
            assert_eq!(
                inspection.metadata.okf_version, case.okf_version,
                "{}",
                case.relative_path
            );
            let actual_codes = inspection
                .findings
                .iter()
                .map(|finding| finding.code.clone())
                .collect::<HashSet<_>>();
            let expected_codes = case.finding_codes.into_iter().collect::<HashSet<_>>();
            assert_eq!(
                actual_codes, expected_codes,
                "{} finding contract changed",
                case.relative_path
            );
        }
    }

    #[test]
    fn v01_normalizes_legacy_metadata() {
        let inspection = inspect_fixture("v01", "concepts/construct.md");
        assert_eq!(inspection.metadata.r#type.as_deref(), Some("Knowledge Map"));
        assert_eq!(inspection.metadata.timestamp.as_deref(), Some("2026-07-20"));
        assert_eq!(
            inspection.metadata.effective_timestamp.as_deref(),
            Some("2026-07-20")
        );
        assert_eq!(inspection.metadata.tags, ["construct", "agents"]);
        assert!(inspection.is_conformant);
    }

    #[test]
    fn v02_preserves_nested_and_unknown_metadata() {
        let inspection = inspect_fixture("v02", "concepts/revenue.md");
        assert_eq!(
            inspection.metadata.effective_timestamp.as_deref(),
            Some("2026-07-25T12:00:00Z")
        );
        assert!(matches!(
            inspection.metadata.sources,
            Some(OkfValue::Sequence { .. })
        ));
        assert!(matches!(
            inspection.metadata.generated,
            Some(OkfValue::Mapping { .. })
        ));
        assert!(inspection
            .metadata
            .extra
            .iter()
            .any(|entry| entry.name == "domain"));
        assert!(inspection.is_conformant);
    }

    #[test]
    fn future_versions_remain_readable() {
        let inspection = inspect_fixture("future", "concept.md");
        assert!(inspection.is_conformant);
        assert!(inspection
            .findings
            .iter()
            .any(|finding| finding.code == "OKF_VERSION_UNSUPPORTED"));
    }

    #[test]
    fn malformed_frontmatter_has_a_stable_finding() {
        let inspection = inspect_fixture("partial", "malformed.md");
        assert!(!inspection.is_conformant);
        assert!(inspection
            .findings
            .iter()
            .any(|finding| finding.code == "OKF_YAML_INVALID"));
    }

    #[test]
    fn review_comments_do_not_create_graph_links() {
        let root = Path::new("/bundle");
        let source = root.join("concept.md");
        let content = "---\ntype: Note\n---\n<!-- construct-review:v1\n{\"comments\":[{\"quote\":\"[hidden](/hidden.md)\"}]}\n-->\n[visible](/visible.md)\n";
        let inspection = inspect(content, "concept.md", &source, root, false);
        assert_eq!(inspection.links.len(), 1);
        assert_eq!(inspection.links[0].target, "/visible.md");
    }

    #[test]
    fn extracts_reference_links_and_known_metadata_paths() {
        let root = Path::new("/bundle");
        let source = root.join("concept.md");
        let content = r#"---
type: Attested Computation
sources:
  - id: input
    resource: ./inputs/source.md
executor:
  resource: references/run.md
custom_path: ./not-a-contract.md
---
[Source document][source]

[source]: ./related.md
"#;
        let inspection = inspect(content, "concept.md", &source, root, false);
        assert!(inspection
            .links
            .iter()
            .any(|link| link.origin == LinkOrigin::Markdown && link.target == "./related.md"));
        assert!(inspection.links.iter().any(|link| {
            link.origin == LinkOrigin::Metadata
                && link.field.as_deref() == Some("sources[0].resource")
                && link.target == "./inputs/source.md"
        }));
        assert!(inspection.links.iter().any(|link| {
            link.origin == LinkOrigin::Metadata
                && link.field.as_deref() == Some("executor.resource")
        }));
        assert!(!inspection
            .links
            .iter()
            .any(|link| link.field.as_deref() == Some("custom_path")));
    }

    #[test]
    fn root_relative_links_cannot_escape_the_bundle() {
        let root = Path::new("/bundle");
        let source = root.join("nested/source.md");
        let link = resolve_link(
            "../../../secret.md",
            &source,
            root,
            LinkOrigin::Markdown,
            None,
            None,
        );
        assert_eq!(link.status, LinkStatus::OutsideBundle);
        assert!(link.resolved_path.is_none());
    }

    #[test]
    fn bundle_fixture_detects_links_and_broken_links() {
        let root = fixture_root("v02");
        let files = ["index.md", "concepts/customer.md", "concepts/revenue.md"]
            .into_iter()
            .map(|relative_path| BundleFile {
                path: root.join(relative_path),
                relative_path: relative_path.to_string(),
            })
            .collect();
        let snapshot = inspect_bundle(&root, files).expect("inspect fixture bundle");
        assert!(snapshot.detected);
        assert_eq!(snapshot.declared_version.as_deref(), Some("0.2"));
        assert_eq!(snapshot.concepts.len(), 2);
        assert!(snapshot
            .findings
            .iter()
            .any(|finding| finding.code == "OKF_LINK_BROKEN"));
        let revenue = snapshot
            .concepts
            .iter()
            .find(|concept| concept.id == "concepts/revenue")
            .expect("revenue concept");
        assert_eq!(revenue.outgoing_paths.len(), 1);
    }

    #[test]
    fn typed_documents_without_a_root_index_do_not_trigger_auto_detection() {
        let root = fixture_root("v02");
        let files = ["concepts/customer.md", "concepts/revenue.md"]
            .into_iter()
            .map(|relative_path| BundleFile {
                path: root.join(relative_path),
                relative_path: relative_path.to_string(),
            })
            .collect();
        let snapshot = inspect_bundle(&root, files).expect("inspect fixture documents");
        assert!(!snapshot.detected);
        assert_eq!(snapshot.concepts.len(), 2);
    }

    #[test]
    #[ignore = "capacity probe; run with cargo test okf::tests::parses_10k_documents -- --ignored --nocapture"]
    fn parses_10k_documents() {
        let root = Path::new("/bundle");
        let started = Instant::now();
        for index in 0..10_000 {
            let relative = format!("concepts/concept-{index}.md");
            let source = root.join(&relative);
            let content = format!(
                "---\ntype: Synthetic\ntitle: Concept {index}\ntags: [benchmark]\ngenerated: {{ by: process:test, at: 2026-07-26T00:00:00Z }}\n---\n# Concept {index}\n\n[Next](./concept-{}.md)\n",
                index + 1
            );
            let inspection = inspect(&content, &relative, &source, root, false);
            assert!(inspection.is_conformant);
        }
        eprintln!(
            "Parsed 10,000 synthetic documents in {:?}",
            started.elapsed()
        );
    }
}
