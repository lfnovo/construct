use crate::{
    okf::{inspect_bundle, BundleFile, FindingSeverity, OkfFinding, SourceRange},
    IGNORED_DIRECTORIES,
};
use glob::{MatchOptions, Pattern};
use serde::Serialize;
use std::{
    cmp::Ordering,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
};
use walkdir::{DirEntry, WalkDir};

const DEFAULT_MAX_FINDINGS: usize = 1_000;
const MAX_CONFIGURED_FINDINGS: usize = 100_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FailOn {
    Error,
    Warning,
    Never,
}

#[derive(Debug, Eq, PartialEq)]
struct CommandOptions {
    root: PathBuf,
    format: OutputFormat,
    fail_on: FailOn,
    excludes: Vec<String>,
    max_findings: usize,
    no_color: bool,
    quiet: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum FindingTier {
    Conformance,
    Compatibility,
    Hygiene,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct Position {
    line: usize,
    column: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct LintRange {
    start: Position,
    end: Position,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct LintFinding {
    code: String,
    severity: FindingSeverity,
    tier: FindingTier,
    relative_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    range: Option<LintRange>,
    message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LintReport {
    bundle_name: String,
    declared_okf_version: Option<String>,
    documents: usize,
    findings: Vec<LintFinding>,
}

#[derive(Debug)]
struct LintExecution {
    report: LintReport,
    failed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonTool<'a> {
    name: &'a str,
    version: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonBundle<'a> {
    name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    declared_okf_version: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonSummary {
    documents: usize,
    errors: usize,
    warnings: usize,
    info: usize,
    truncated: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonReport<'a> {
    schema_version: u8,
    tool: JsonTool<'a>,
    bundle: JsonBundle<'a>,
    summary: JsonSummary,
    findings: &'a [LintFinding],
}

fn usage() -> &'static str {
    "Usage: construct okf lint [PATH] [OPTIONS]\n\
\n\
Validate an OKF bundle without modifying it or creating Construct state.\n\
\n\
Arguments:\n\
  PATH                       Bundle directory (default: current directory)\n\
\n\
Options:\n\
  --format <text|json>       Output format (default: text)\n\
  --fail-on <error|warning|never>\n\
                             Finding threshold for exit code 1 (default: error)\n\
  --exclude <GLOB>           Exclude a relative path pattern (repeatable)\n\
  --max-findings <COUNT>     Maximum findings included in output (default: 1000)\n\
  --no-color                 Disable terminal colors\n\
  --quiet                    Suppress individual findings\n\
  -h, --help                 Show this help\n"
}

fn parse_format(value: &str) -> Result<OutputFormat, String> {
    match value {
        "text" => Ok(OutputFormat::Text),
        "json" => Ok(OutputFormat::Json),
        _ => Err(format!(
            "invalid --format value '{value}'; expected text or json"
        )),
    }
}

fn parse_fail_on(value: &str) -> Result<FailOn, String> {
    match value {
        "error" => Ok(FailOn::Error),
        "warning" => Ok(FailOn::Warning),
        "never" => Ok(FailOn::Never),
        _ => Err(format!(
            "invalid --fail-on value '{value}'; expected error, warning, or never"
        )),
    }
}

fn option_value(arguments: &[String], index: &mut usize, option: &str) -> Result<String, String> {
    *index += 1;
    arguments
        .get(*index)
        .cloned()
        .ok_or_else(|| format!("{option} requires a value"))
}

fn parse_options(arguments: &[String]) -> Result<Option<CommandOptions>, String> {
    if arguments.is_empty()
        || matches!(arguments.first().map(String::as_str), Some("-h" | "--help"))
    {
        return Ok(None);
    }
    if arguments.first().map(String::as_str) != Some("lint") {
        return Err(format!(
            "unknown command '{}'\n\n{}",
            arguments.first().map(String::as_str).unwrap_or_default(),
            usage()
        ));
    }
    if arguments
        .get(1)
        .is_some_and(|argument| matches!(argument.as_str(), "-h" | "--help"))
    {
        return Ok(None);
    }

    let mut root = None;
    let mut format = OutputFormat::Text;
    let mut fail_on = FailOn::Error;
    let mut excludes = Vec::new();
    let mut max_findings = DEFAULT_MAX_FINDINGS;
    let mut no_color = false;
    let mut quiet = false;
    let mut positional_only = false;
    let mut index = 1;
    while index < arguments.len() {
        let argument = &arguments[index];
        if positional_only {
            if root.replace(PathBuf::from(argument)).is_some() {
                return Err("only one bundle PATH may be supplied".to_string());
            }
        } else if argument == "--" {
            positional_only = true;
        } else if argument == "--no-color" {
            no_color = true;
        } else if argument == "--quiet" {
            quiet = true;
        } else if let Some(value) = argument.strip_prefix("--format=") {
            format = parse_format(value)?;
        } else if argument == "--format" {
            format = parse_format(&option_value(arguments, &mut index, "--format")?)?;
        } else if let Some(value) = argument.strip_prefix("--fail-on=") {
            fail_on = parse_fail_on(value)?;
        } else if argument == "--fail-on" {
            fail_on = parse_fail_on(&option_value(arguments, &mut index, "--fail-on")?)?;
        } else if let Some(value) = argument.strip_prefix("--exclude=") {
            excludes.push(value.to_string());
        } else if argument == "--exclude" {
            excludes.push(option_value(arguments, &mut index, "--exclude")?);
        } else if let Some(value) = argument.strip_prefix("--max-findings=") {
            max_findings = parse_max_findings(value)?;
        } else if argument == "--max-findings" {
            max_findings =
                parse_max_findings(&option_value(arguments, &mut index, "--max-findings")?)?;
        } else if matches!(argument.as_str(), "-h" | "--help") {
            return Ok(None);
        } else if argument.starts_with('-') {
            return Err(format!("unknown option '{argument}'"));
        } else if root.replace(PathBuf::from(argument)).is_some() {
            return Err("only one bundle PATH may be supplied".to_string());
        }
        index += 1;
    }

    Ok(Some(CommandOptions {
        root: root.unwrap_or_else(|| PathBuf::from(".")),
        format,
        fail_on,
        excludes,
        max_findings,
        no_color,
        quiet,
    }))
}

fn parse_max_findings(value: &str) -> Result<usize, String> {
    let count = value
        .parse::<usize>()
        .map_err(|_| format!("invalid --max-findings value '{value}'; expected a count"))?;
    if count > MAX_CONFIGURED_FINDINGS {
        return Err(format!(
            "--max-findings cannot exceed {MAX_CONFIGURED_FINDINGS}"
        ));
    }
    Ok(count)
}

fn compile_excludes(patterns: &[String]) -> Result<Vec<Pattern>, String> {
    patterns
        .iter()
        .map(|pattern| {
            Pattern::new(pattern)
                .map_err(|error| format!("invalid --exclude pattern '{pattern}': {error}"))
        })
        .collect()
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| matches!(extension.to_ascii_lowercase().as_str(), "md" | "markdown"))
        .unwrap_or(false)
}

fn relative_path(root: &Path, path: &Path) -> Option<String> {
    path.strip_prefix(root)
        .ok()
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
}

fn matches_exclusion(excludes: &[Pattern], candidate: &str) -> bool {
    let options = MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    };
    excludes
        .iter()
        .any(|pattern| pattern.matches_with(candidate, options))
}

fn excluded_entry(entry: &DirEntry, root: &Path, excludes: &[Pattern]) -> bool {
    if entry.depth() == 0 {
        return false;
    }
    if entry.file_type().is_dir()
        && entry.file_name().to_str().is_some_and(|name| {
            IGNORED_DIRECTORIES
                .iter()
                .any(|ignored| name.eq_ignore_ascii_case(ignored))
        })
    {
        return true;
    }
    let Some(relative) = relative_path(root, entry.path()) else {
        return true;
    };
    matches_exclusion(excludes, &relative)
        || (entry.file_type().is_dir()
            && (matches_exclusion(excludes, &format!("{relative}/"))
                || matches_exclusion(excludes, &format!("{relative}/__construct_descendant__"))))
}

fn walk_error_finding(root: &Path, error: &walkdir::Error) -> LintFinding {
    let relative = error
        .path()
        .and_then(|path| relative_path(root, path))
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| ".".to_string());
    LintFinding {
        code: "OKF_PATH_UNREADABLE".to_string(),
        severity: FindingSeverity::Error,
        tier: FindingTier::Conformance,
        relative_path: relative,
        range: None,
        message: error
            .io_error()
            .map(|error| format!("Could not inspect this path: {error}"))
            .unwrap_or_else(|| "Could not inspect this path.".to_string()),
    }
}

fn discover_files(
    root: &Path,
    excludes: &[Pattern],
) -> Result<(Vec<BundleFile>, Vec<LintFinding>), String> {
    let mut files = Vec::new();
    let mut findings = Vec::new();
    let walker = WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !excluded_entry(entry, root, excludes));
    for entry in walker {
        let entry = match entry {
            Ok(entry) => entry,
            Err(error) => {
                findings.push(walk_error_finding(root, &error));
                continue;
            }
        };
        if !entry.file_type().is_file() || !is_markdown(entry.path()) {
            continue;
        }
        let Some(relative_path) = relative_path(root, entry.path()) else {
            continue;
        };
        files.push(BundleFile {
            path: entry.into_path(),
            relative_path,
        });
    }
    files.sort_by(|left, right| {
        left.relative_path
            .to_ascii_lowercase()
            .cmp(&right.relative_path.to_ascii_lowercase())
            .then_with(|| left.relative_path.cmp(&right.relative_path))
    });
    Ok((files, findings))
}

fn finding_tier(code: &str) -> FindingTier {
    match code {
        "OKF_FRONTMATTER_REQUIRED"
        | "OKF_FRONTMATTER_UNCLOSED"
        | "OKF_FRONTMATTER_TOO_LARGE"
        | "OKF_FRONTMATTER_NOT_MAPPING"
        | "OKF_YAML_INVALID"
        | "OKF_YAML_DEPTH_EXCEEDED"
        | "OKF_TYPE_REQUIRED"
        | "OKF_DOCUMENT_TOO_LARGE"
        | "OKF_FILE_UNREADABLE"
        | "OKF_PATH_UNREADABLE"
        | "OKF_LOG_FRONTMATTER"
        | "OKF_LOG_DATE_HEADING_REQUIRED"
        | "OKF_INDEX_FRONTMATTER"
        | "OKF_LINK_OUTSIDE_BUNDLE" => FindingTier::Conformance,
        "OKF_LINK_BROKEN" => FindingTier::Hygiene,
        _ => FindingTier::Compatibility,
    }
}

fn lint_range(range: SourceRange) -> LintRange {
    LintRange {
        start: Position {
            line: range.start_line,
            column: range.start_column,
        },
        end: Position {
            line: range.end_line,
            column: range.end_column,
        },
    }
}

fn lint_finding(finding: OkfFinding) -> LintFinding {
    LintFinding {
        tier: finding_tier(&finding.code),
        code: finding.code,
        severity: finding.severity,
        relative_path: finding.relative_path,
        range: finding.range.map(lint_range),
        message: finding.message,
    }
}

fn severity_rank(severity: FindingSeverity) -> u8 {
    match severity {
        FindingSeverity::Error => 0,
        FindingSeverity::Warning => 1,
        FindingSeverity::Info => 2,
    }
}

fn compare_findings(left: &LintFinding, right: &LintFinding) -> Ordering {
    severity_rank(left.severity)
        .cmp(&severity_rank(right.severity))
        .then_with(|| {
            left.relative_path
                .to_ascii_lowercase()
                .cmp(&right.relative_path.to_ascii_lowercase())
        })
        .then_with(|| left.relative_path.cmp(&right.relative_path))
        .then_with(|| {
            left.range
                .as_ref()
                .map(|range| (range.start.line, range.start.column))
                .unwrap_or((usize::MAX, usize::MAX))
                .cmp(
                    &right
                        .range
                        .as_ref()
                        .map(|range| (range.start.line, range.start.column))
                        .unwrap_or((usize::MAX, usize::MAX)),
                )
        })
        .then_with(|| left.code.cmp(&right.code))
        .then_with(|| left.message.cmp(&right.message))
}

fn lint_bundle(root: &Path, exclude_patterns: &[String]) -> Result<LintReport, String> {
    let root = root
        .canonicalize()
        .map_err(|error| format!("could not access '{}': {error}", root.display()))?;
    if !root.is_dir() {
        return Err(format!("'{}' is not a directory", root.display()));
    }
    let excludes = compile_excludes(exclude_patterns)?;
    let (files, mut findings) = discover_files(&root, &excludes)?;
    let snapshot = inspect_bundle(&root, files)?;
    findings.extend(snapshot.findings.into_iter().map(lint_finding));
    findings.sort_by(compare_findings);
    let bundle_name = root
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(".")
        .to_string();
    Ok(LintReport {
        bundle_name,
        declared_okf_version: snapshot.declared_version,
        documents: snapshot.document_count,
        findings,
    })
}

fn finding_counts(findings: &[LintFinding]) -> (usize, usize, usize) {
    findings.iter().fold(
        (0, 0, 0),
        |(errors, warnings, info), finding| match finding.severity {
            FindingSeverity::Error => (errors + 1, warnings, info),
            FindingSeverity::Warning => (errors, warnings + 1, info),
            FindingSeverity::Info => (errors, warnings, info + 1),
        },
    )
}

fn fails(report: &LintReport, fail_on: FailOn) -> bool {
    let (errors, warnings, _) = finding_counts(&report.findings);
    match fail_on {
        FailOn::Error => errors > 0,
        FailOn::Warning => errors > 0 || warnings > 0,
        FailOn::Never => false,
    }
}

fn displayed_findings<'a>(report: &'a LintReport, options: &CommandOptions) -> &'a [LintFinding] {
    if options.quiet {
        &[]
    } else {
        &report.findings[..report.findings.len().min(options.max_findings)]
    }
}

fn severity_label(severity: FindingSeverity) -> &'static str {
    match severity {
        FindingSeverity::Error => "ERROR",
        FindingSeverity::Warning => "WARNING",
        FindingSeverity::Info => "INFO",
    }
}

fn colored_severity(severity: FindingSeverity, color: bool) -> String {
    let label = severity_label(severity);
    if !color {
        return label.to_string();
    }
    let code = match severity {
        FindingSeverity::Error => 31,
        FindingSeverity::Warning => 33,
        FindingSeverity::Info => 36,
    };
    format!("\u{1b}[{code}m{label}\u{1b}[0m")
}

fn counted(count: usize, singular: &str, plural: &str) -> String {
    format!("{count} {}", if count == 1 { singular } else { plural })
}

fn render_text(execution: &LintExecution, options: &CommandOptions, color: bool) -> String {
    let report = &execution.report;
    let displayed = displayed_findings(report, options);
    let mut output = format!("OKF lint: {}\n", report.bundle_name);
    for finding in displayed {
        let location = finding
            .range
            .as_ref()
            .map(|range| {
                format!(
                    "{}:{}:{}",
                    finding.relative_path, range.start.line, range.start.column
                )
            })
            .unwrap_or_else(|| finding.relative_path.clone());
        output.push_str(&format!(
            "\n{} {} {}\n  {}\n",
            colored_severity(finding.severity, color),
            finding.code,
            location,
            finding.message
        ));
    }
    if displayed.len() < report.findings.len() {
        if options.quiet {
            output.push_str("\nFindings omitted by --quiet.\n");
        } else {
            output.push_str(&format!(
                "\nOutput limited to {} of {} findings.\n",
                displayed.len(),
                report.findings.len()
            ));
        }
    }
    let (errors, warnings, info) = finding_counts(&report.findings);
    output.push_str(&format!(
        "\nSummary: {} · {} · {} · {}\nResult: {}\n",
        counted(report.documents, "document", "documents"),
        counted(errors, "error", "errors"),
        counted(warnings, "warning", "warnings"),
        counted(info, "info", "info"),
        if execution.failed { "failed" } else { "passed" }
    ));
    output
}

fn render_json(execution: &LintExecution, options: &CommandOptions) -> Result<String, String> {
    let report = &execution.report;
    let findings = displayed_findings(report, options);
    let (errors, warnings, info) = finding_counts(&report.findings);
    let value = JsonReport {
        schema_version: 1,
        tool: JsonTool {
            name: "construct-okf-lint",
            version: env!("CARGO_PKG_VERSION"),
        },
        bundle: JsonBundle {
            name: &report.bundle_name,
            declared_okf_version: report.declared_okf_version.as_deref(),
        },
        summary: JsonSummary {
            documents: report.documents,
            errors,
            warnings,
            info,
            truncated: findings.len() < report.findings.len(),
        },
        findings,
    };
    serde_json::to_string_pretty(&value)
        .map(|output| format!("{output}\n"))
        .map_err(|error| format!("could not serialize the JSON report: {error}"))
}

fn execute(options: &CommandOptions) -> Result<LintExecution, String> {
    let report = lint_bundle(&options.root, &options.excludes)?;
    let failed = fails(&report, options.fail_on);
    Ok(LintExecution { report, failed })
}

fn write_stdout(output: &str) -> Result<(), String> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    stdout
        .write_all(output.as_bytes())
        .and_then(|_| stdout.flush())
        .map_err(|error| format!("could not write the lint report: {error}"))
}

pub(crate) fn run_command(arguments: &[String]) -> Result<i32, String> {
    let Some(options) = parse_options(arguments)? else {
        write_stdout(usage())?;
        return Ok(0);
    };
    let execution = execute(&options)?;
    let output = match options.format {
        OutputFormat::Text => render_text(
            &execution,
            &options,
            !options.no_color && io::stdout().is_terminal(),
        ),
        OutputFormat::Json => render_json(&execution, &options)?,
    };
    write_stdout(&output)?;
    Ok(i32::from(execution.failed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        time::{Instant, SystemTime, UNIX_EPOCH},
    };

    fn fixture_root(case: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/okf")
            .join(case)
    }

    fn options(root: PathBuf) -> CommandOptions {
        CommandOptions {
            root,
            format: OutputFormat::Text,
            fail_on: FailOn::Error,
            excludes: Vec::new(),
            max_findings: DEFAULT_MAX_FINDINGS,
            no_color: true,
            quiet: false,
        }
    }

    fn temporary_directory(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "construct-okf-lint-{}-{name}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temporary directory");
        path
    }

    #[test]
    fn parses_public_cli_options_in_any_order() {
        let arguments = [
            "lint",
            "--format=json",
            "--exclude",
            "drafts/**",
            ".",
            "--fail-on",
            "warning",
            "--max-findings=25",
            "--no-color",
            "--quiet",
        ]
        .map(str::to_string);
        let parsed = parse_options(&arguments)
            .expect("parse options")
            .expect("command options");
        assert_eq!(parsed.root, PathBuf::from("."));
        assert_eq!(parsed.format, OutputFormat::Json);
        assert_eq!(parsed.fail_on, FailOn::Warning);
        assert_eq!(parsed.excludes, ["drafts/**"]);
        assert_eq!(parsed.max_findings, 25);
        assert!(parsed.no_color);
        assert!(parsed.quiet);
    }

    #[test]
    fn v02_fixture_produces_deterministic_machine_readable_output() {
        let mut options = options(fixture_root("v02"));
        options.format = OutputFormat::Json;
        let execution = execute(&options).expect("lint fixture");
        let first = render_json(&execution, &options).expect("render JSON");
        let second = render_json(&execution, &options).expect("render JSON again");
        assert_eq!(first, second);
        let value: serde_json::Value = serde_json::from_str(&first).expect("valid JSON");
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["bundle"]["declaredOkfVersion"], "0.2");
        assert_eq!(value["summary"]["documents"], 3);
        assert_eq!(value["summary"]["errors"], 0);
        assert_eq!(value["summary"]["warnings"], 1);
        assert!(!execution.failed);
    }

    #[test]
    fn error_threshold_and_warning_threshold_have_distinct_results() {
        let root = fixture_root("v02");
        let default = execute(&options(root.clone())).expect("default lint");
        assert!(!default.failed);
        let mut strict_options = options(root);
        strict_options.fail_on = FailOn::Warning;
        let strict = execute(&strict_options).expect("strict lint");
        assert!(strict.failed);
    }

    #[test]
    fn explicit_globs_exclude_matching_subtrees() {
        let root = temporary_directory("exclude");
        fs::write(root.join("valid.md"), "---\ntype: Note\n---\n# Valid\n")
            .expect("write valid file");
        fs::create_dir_all(root.join("drafts")).expect("create drafts");
        fs::write(root.join("drafts/invalid.md"), "# Missing frontmatter\n")
            .expect("write invalid draft");

        let report = lint_bundle(&root, &["drafts/**".to_string()]).expect("lint bundle");
        assert_eq!(report.documents, 1);
        assert!(report.findings.is_empty());
        fs::remove_dir_all(root).expect("remove temporary directory");
    }

    #[test]
    fn output_limit_does_not_change_counts_or_exit_behavior() {
        let root = temporary_directory("limit");
        fs::write(root.join("one.md"), "# One\n").expect("write one");
        fs::write(root.join("two.md"), "# Two\n").expect("write two");
        let mut options = options(root.clone());
        options.max_findings = 1;
        let execution = execute(&options).expect("lint bundle");
        let json = render_json(&execution, &options).expect("render JSON");
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid JSON");
        assert_eq!(value["summary"]["errors"], 2);
        assert_eq!(value["findings"].as_array().expect("findings").len(), 1);
        assert_eq!(value["summary"]["truncated"], true);
        assert!(execution.failed);
        fs::remove_dir_all(root).expect("remove temporary directory");
    }

    #[test]
    fn normal_text_output_never_contains_the_absolute_root() {
        let root = temporary_directory("relative");
        fs::write(root.join("invalid.md"), "# Missing frontmatter\n").expect("write invalid");
        let options = options(root.clone());
        let execution = execute(&options).expect("lint bundle");
        let text = render_text(&execution, &options, false);
        assert!(text.contains("OKF_FRONTMATTER_REQUIRED invalid.md"));
        assert!(!text.contains(&root.to_string_lossy().to_string()));
        fs::remove_dir_all(root).expect("remove temporary directory");
    }

    #[test]
    #[ignore = "capacity probe; run with cargo test okf_lint::tests::lints_10k_documents -- --ignored --nocapture"]
    fn lints_10k_documents() {
        let root = temporary_directory("10k");
        fs::write(
            root.join("index.md"),
            "---\nokf_version: \"0.2\"\n---\n# Synthetic bundle\n",
        )
        .expect("write root index");
        for index in 0..9_999 {
            fs::write(
                root.join(format!("concept-{index}.md")),
                format!(
                    "---\ntype: Synthetic\ntitle: Concept {index}\ndescription: Synthetic capacity fixture.\n---\n# Concept {index}\n"
                ),
            )
            .expect("write concept");
        }
        let started = Instant::now();
        let report = lint_bundle(&root, &[]).expect("lint synthetic bundle");
        assert_eq!(report.documents, 10_000);
        assert!(report.findings.is_empty());
        eprintln!(
            "Linted 10,000 synthetic documents in {:?}",
            started.elapsed()
        );
        fs::remove_dir_all(root).expect("remove temporary directory");
    }
}
