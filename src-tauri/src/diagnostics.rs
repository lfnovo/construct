use chrono::Utc;
use serde_json::{Map, Value};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

const MAX_LOG_BYTES: u64 = 1024 * 1024;
const RETAINED_LOGS: usize = 2;

#[derive(Clone)]
pub(crate) struct Diagnostics {
    data_dir: PathBuf,
    component: &'static str,
    enabled: bool,
}

impl Diagnostics {
    pub(crate) fn new(data_dir: PathBuf, component: &'static str) -> Self {
        Self {
            data_dir,
            component,
            enabled: true,
        }
    }

    #[cfg(test)]
    pub(crate) fn disabled() -> Self {
        Self {
            data_dir: PathBuf::new(),
            component: "disabled",
            enabled: false,
        }
    }

    pub(crate) fn info(&self, event: &str, fields: Value) {
        self.write("info", event, fields);
    }

    pub(crate) fn warn(&self, event: &str, fields: Value) {
        self.write("warn", event, fields);
    }

    pub(crate) fn failure(&self, event: &str, fields: Value) {
        self.write("error", event, fields);
    }

    pub(crate) fn error(&self, event: &str, error: &str, fields: Value) {
        let mut fields = object_fields(fields);
        fields.insert(
            "error".to_string(),
            Value::String(self.sanitize_message(error)),
        );
        self.write("error", event, Value::Object(fields));
    }

    pub(crate) fn location_identity(location_id: &str) -> String {
        blake3::hash(location_id.as_bytes()).to_hex()[..12].to_string()
    }

    fn write(&self, level: &str, event: &str, fields: Value) {
        if !self.enabled {
            return;
        }
        let log_dir = self.data_dir.join("logs");
        if fs::create_dir_all(&log_dir).is_err() {
            return;
        }
        protect_directory(&log_dir);
        let path = log_dir.join(format!("{}.log", self.component));
        if rotate_if_needed(&path).is_err() {
            return;
        }

        let mut record = object_fields(fields);
        record.insert(
            "timestamp".to_string(),
            Value::String(Utc::now().to_rfc3339()),
        );
        record.insert("level".to_string(), Value::String(level.to_string()));
        record.insert(
            "component".to_string(),
            Value::String(self.component.to_string()),
        );
        record.insert("event".to_string(), Value::String(event.to_string()));
        record.insert(
            "appVersion".to_string(),
            Value::String(env!("CARGO_PKG_VERSION").to_string()),
        );
        record.insert(
            "os".to_string(),
            Value::String(std::env::consts::OS.to_string()),
        );
        record.insert(
            "arch".to_string(),
            Value::String(std::env::consts::ARCH.to_string()),
        );
        record.insert("pid".to_string(), Value::Number(std::process::id().into()));

        let Ok(mut encoded) = serde_json::to_vec(&Value::Object(record)) else {
            return;
        };
        encoded.push(b'\n');
        let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) else {
            return;
        };
        protect_file(&path);
        let _ = file.write_all(&encoded);
    }

    fn sanitize_message(&self, message: &str) -> String {
        let mut sanitized =
            message.replace(&self.data_dir.to_string_lossy().to_string(), "<data-dir>");
        let variables = [
            "HOME",
            "USERPROFILE",
            "APPDATA",
            "LOCALAPPDATA",
            "TEMP",
            "TMP",
        ];
        for variable in variables {
            if let Some(value) = std::env::var_os(variable) {
                let value = PathBuf::from(value).to_string_lossy().to_string();
                if !value.is_empty() {
                    sanitized =
                        sanitized.replace(&value, &format!("<{}>", variable.to_ascii_lowercase()));
                }
            }
        }
        if let Ok(executable) = std::env::current_exe() {
            sanitized = sanitized.replace(
                &executable.to_string_lossy().to_string(),
                "<construct-executable>",
            );
        }
        truncate(&sanitized, 4_000)
    }
}

fn object_fields(fields: Value) -> Map<String, Value> {
    match fields {
        Value::Object(fields) => fields,
        _ => Map::from_iter([("detail".to_string(), fields)]),
    }
}

fn rotate_if_needed(path: &Path) -> std::io::Result<()> {
    if path.metadata().map(|metadata| metadata.len()).unwrap_or(0) < MAX_LOG_BYTES {
        return Ok(());
    }
    for generation in (1..=RETAINED_LOGS).rev() {
        let destination = path.with_extension(format!("log.{generation}"));
        if generation == RETAINED_LOGS && destination.exists() {
            fs::remove_file(&destination)?;
        }
        let source = if generation == 1 {
            path.to_path_buf()
        } else {
            path.with_extension(format!("log.{}", generation - 1))
        };
        if source.exists() {
            fs::rename(source, destination)?;
        }
    }
    Ok(())
}

fn truncate(value: &str, max_characters: usize) -> String {
    let mut characters = value.chars();
    let truncated = characters.by_ref().take(max_characters).collect::<String>();
    if characters.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

#[cfg(unix)]
fn protect_directory(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
}

#[cfg(not(unix))]
fn protect_directory(_path: &Path) {}

#[cfg(unix)]
fn protect_file(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn protect_file(_path: &Path) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_root() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "construct-diagnostics-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&path).expect("create diagnostics test directory");
        path
    }

    #[test]
    fn writes_sanitized_json_lines_without_user_paths() {
        let root = temporary_root();
        let diagnostics = Diagnostics::new(root.clone(), "construct-test");
        diagnostics.error(
            "search_failed",
            &format!("Could not open {}", root.join("private").display()),
            serde_json::json!({
                "location": Diagnostics::location_identity("location-secret"),
                "queryCharacters": 12
            }),
        );
        let log =
            fs::read_to_string(root.join("logs/construct-test.log")).expect("read diagnostics log");
        assert!(log.contains("\"event\":\"search_failed\""));
        assert!(log.contains("\"queryCharacters\":12"));
        assert!(log.contains("<data-dir>"));
        assert!(!log.contains(root.to_string_lossy().as_ref()));
        assert!(!log.contains("location-secret"));
        fs::remove_dir_all(root).expect("remove diagnostics test directory");
    }

    #[test]
    fn location_identity_is_short_and_stable() {
        let first = Diagnostics::location_identity("location-one");
        let second = Diagnostics::location_identity("location-one");
        assert_eq!(first, second);
        assert_eq!(first.len(), 12);
        assert_ne!(first, Diagnostics::location_identity("location-two"));
    }

    #[test]
    fn rotates_bounded_logs_and_keeps_the_previous_generation() {
        let root = temporary_root();
        let log_dir = root.join("logs");
        fs::create_dir_all(&log_dir).expect("create log directory");
        let log_path = log_dir.join("construct-test.log");
        fs::write(&log_path, vec![b'x'; MAX_LOG_BYTES as usize]).expect("write oversized log");
        let diagnostics = Diagnostics::new(root.clone(), "construct-test");
        diagnostics.info("after_rotation", serde_json::json!({}));
        assert!(log_dir.join("construct-test.log.1").exists());
        let current = fs::read_to_string(log_path).expect("read current log");
        assert!(current.contains("\"event\":\"after_rotation\""));
        fs::remove_dir_all(root).expect("remove diagnostics test directory");
    }
}
