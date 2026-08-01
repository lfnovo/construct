use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum DesktopOpenKind {
    Directory,
    File,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DesktopOpenRequest {
    pub(crate) kind: DesktopOpenKind,
    pub(crate) path: String,
}

pub(crate) fn forwarded_arguments(arguments: &[String]) -> &[String] {
    arguments.get(1..).unwrap_or_default()
}

pub(crate) fn parse_request(
    arguments: &[String],
    current_directory: &Path,
) -> Result<Option<DesktopOpenRequest>, String> {
    let arguments = arguments
        .iter()
        .filter(|argument| !argument.starts_with("-psn_"))
        .collect::<Vec<_>>();
    if arguments.is_empty() {
        return Ok(None);
    }
    if arguments.len() > 1 {
        return Err("Open one Location or Markdown file at a time.".to_string());
    }
    let argument = arguments[0];
    if argument.starts_with('-') {
        return Err(format!("Unknown desktop option: {argument}"));
    }
    let candidate = PathBuf::from(argument);
    let candidate = if candidate.is_absolute() {
        candidate
    } else {
        current_directory.join(candidate)
    };
    let canonical = candidate
        .canonicalize()
        .map_err(|error| format!("Could not open '{}': {error}", candidate.display()))?;
    let kind = if canonical.is_dir() {
        DesktopOpenKind::Directory
    } else if canonical.is_file() && is_markdown(&canonical) {
        DesktopOpenKind::File
    } else if canonical.is_file() {
        return Err("Construct can open only folders and Markdown files.".to_string());
    } else {
        return Err("The requested path is not a folder or file.".to_string());
    };
    Ok(Some(DesktopOpenRequest {
        kind,
        path: canonical.to_string_lossy().to_string(),
    }))
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            extension.eq_ignore_ascii_case("md") || extension.eq_ignore_ascii_case("markdown")
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEMPORARY_ROOT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn temporary_root() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "construct-desktop-open-{}-{}",
            std::process::id(),
            TEMPORARY_ROOT_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temporary directory");
        path
    }

    fn arguments(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_owned()).collect()
    }

    #[test]
    fn resolves_relative_directories_and_markdown_files() {
        let root = temporary_root();
        fs::create_dir_all(root.join("notes")).expect("create notes directory");
        fs::write(root.join("notes/context.md"), "# Context").expect("create Markdown file");

        let directory = parse_request(&arguments(&["notes"]), &root)
            .expect("parse directory")
            .expect("directory request");
        assert_eq!(directory.kind, DesktopOpenKind::Directory);
        assert_eq!(
            directory.path,
            root.join("notes")
                .canonicalize()
                .expect("canonicalize notes directory")
                .to_string_lossy()
        );

        let file = parse_request(&arguments(&["notes/context.md"]), &root)
            .expect("parse file")
            .expect("file request");
        assert_eq!(file.kind, DesktopOpenKind::File);
        assert_eq!(
            file.path,
            root.join("notes/context.md")
                .canonicalize()
                .expect("canonicalize Markdown file")
                .to_string_lossy()
        );

        fs::remove_dir_all(root).expect("remove temporary directory");
    }

    #[test]
    fn rejects_missing_unsupported_and_multiple_paths() {
        let root = temporary_root();
        fs::write(root.join("notes.txt"), "Notes").expect("create text file");

        assert!(parse_request(&arguments(&["missing.md"]), &root).is_err());
        assert!(parse_request(&arguments(&["notes.txt"]), &root).is_err());
        assert!(parse_request(&arguments(&["one.md", "two.md"]), &root).is_err());
        assert!(parse_request(&arguments(&["--source"]), &root).is_err());

        fs::remove_dir_all(root).expect("remove temporary directory");
    }

    #[test]
    fn ignores_the_macos_process_serial_number() {
        let root = temporary_root();
        assert_eq!(
            parse_request(&arguments(&["-psn_0_12345"]), &root).expect("parse Finder launch"),
            None
        );
        fs::remove_dir_all(root).expect("remove temporary directory");
    }

    #[test]
    fn removes_the_executable_from_forwarded_arguments() {
        let values = arguments(&["/Applications/Construct.app/Contents/MacOS/construct", "."]);
        assert_eq!(forwarded_arguments(&values), &[".".to_string()]);
    }
}
