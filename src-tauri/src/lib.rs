use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::Utc;
use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
};
use tauri::{AppHandle, Emitter, Manager, State};
use walkdir::{DirEntry, WalkDir};

mod index;
mod knowledge;
mod mcp;
mod okf;

const IGNORED_DIRECTORIES: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "vendor",
    ".venv",
    "venv",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    "target",
    "dist",
    "build",
    "out",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".gradle",
    ".idea",
    "Pods",
    "DerivedData",
    "bin",
    "obj",
    ".terraform",
    ".dart_tool",
    ".pub-cache",
    "coverage",
    ".coverage",
];

#[derive(Default)]
struct WatchState {
    roots: Mutex<Vec<PathBuf>>,
    watchers: Mutex<Vec<RecommendedWatcher>>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct FileEntry {
    path: String,
    relative_path: String,
    name: String,
    modified_at_ms: i64,
    size: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FileContent {
    content: String,
    line_ending: String,
    modified_at_ms: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GitInfo {
    available: bool,
    repo_root: Option<String>,
    status: Option<String>,
    has_head: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GitDiff {
    available: bool,
    diff: String,
    message: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct FsChange {
    paths: Vec<String>,
    kind: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WriteFileRequest {
    path: String,
    content: String,
}

fn app_data_file(app: &AppHandle) -> Result<PathBuf, String> {
    let data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("Could not locate the application data directory: {error}"))?;
    fs::create_dir_all(&data_dir)
        .map_err(|error| format!("Could not create the application data directory: {error}"))?;
    Ok(data_dir.join("workspace.json"))
}

fn legacy_app_data_file(app: &AppHandle) -> Option<PathBuf> {
    let data_dir = app.path().app_data_dir().ok()?;
    Some(
        data_dir
            .parent()?
            .join("com.luisnovo.agent-context")
            .join("workspace.json"),
    )
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| matches!(extension.to_ascii_lowercase().as_str(), "md" | "markdown"))
        .unwrap_or(false)
}

fn is_ignored_entry(entry: &DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }
    entry
        .file_name()
        .to_str()
        .map(|name| {
            IGNORED_DIRECTORIES
                .iter()
                .any(|ignored| name.eq_ignore_ascii_case(ignored))
        })
        .unwrap_or(false)
}

fn timestamp_ms(metadata: &fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_else(|| Utc::now().timestamp_millis())
}

fn normalize_path(path: &str) -> Result<PathBuf, String> {
    let candidate = PathBuf::from(path);
    candidate
        .canonicalize()
        .map_err(|error| format!("Could not access '{}': {error}", candidate.display()))
}

fn is_allowed(path: &Path, state: &WatchState) -> bool {
    let Ok(candidate) = path.canonicalize() else {
        return false;
    };
    let Ok(roots) = state.roots.lock() else {
        return false;
    };
    roots.iter().any(|root| candidate.starts_with(root))
}

fn require_allowed(path: &str, state: &WatchState) -> Result<PathBuf, String> {
    let candidate = normalize_path(path)?;
    if !is_allowed(&candidate, state) {
        return Err("The file is not inside a folder registered in Construct.".to_string());
    }
    Ok(candidate)
}

fn run_git(path: &Path, arguments: &[&str]) -> Result<std::process::Output, String> {
    Command::new("git")
        .current_dir(path.parent().unwrap_or(path))
        .args(arguments)
        .output()
        .map_err(|error| format!("Could not run Git: {error}"))
}

fn git_root(path: &Path) -> Result<PathBuf, String> {
    let output = run_git(path, &["rev-parse", "--show-toplevel"])?;
    if !output.status.success() {
        return Err("This file is not inside a Git repository.".to_string());
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
    normalize_path(&root)
}

fn git_status(path: &Path, root: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).ok()?.to_string_lossy().to_string();
    let output = Command::new("git")
        .current_dir(root)
        .args(["status", "--porcelain=1", "--", &relative])
        .output()
        .ok()?;
    let status = String::from_utf8_lossy(&output.stdout);
    status
        .get(0..2)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn git_has_head(root: &Path) -> bool {
    Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "--verify", "HEAD"])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn collect_files(root: &Path) -> Result<Vec<FileEntry>, String> {
    let mut entries = Vec::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !is_ignored_entry(entry))
    {
        let entry = entry.map_err(|error| format!("Could not walk the folder: {error}"))?;
        if !entry.file_type().is_file() || !is_markdown(entry.path()) {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|error| format!("Could not read file metadata: {error}"))?;
        let relative_path = entry
            .path()
            .strip_prefix(root)
            .map_err(|error| format!("Could not calculate the relative path: {error}"))?
            .to_string_lossy()
            .to_string();
        entries.push(FileEntry {
            path: entry.path().to_string_lossy().to_string(),
            relative_path,
            name: entry.file_name().to_string_lossy().to_string(),
            modified_at_ms: timestamp_ms(&metadata),
            size: metadata.len(),
        });
    }
    entries.sort_by_key(|entry| entry.relative_path.to_lowercase());
    Ok(entries)
}

#[tauri::command]
fn load_app_state(app: AppHandle) -> Result<Value, String> {
    let path = app_data_file(&app)?;
    let source = if path.exists() {
        path
    } else if let Some(legacy) = legacy_app_data_file(&app).filter(|candidate| candidate.exists()) {
        legacy
    } else {
        return Ok(serde_json::json!({}));
    };
    let contents = fs::read_to_string(&source)
        .map_err(|error| format!("Could not read the saved workspace: {error}"))?;
    serde_json::from_str(&contents)
        .map_err(|error| format!("O workspace salvo está inválido: {error}"))
}

#[tauri::command]
fn save_app_state(app: AppHandle, state: Value) -> Result<(), String> {
    let path = app_data_file(&app)?;
    let temporary = path.with_extension("json.tmp");
    let serialized = serde_json::to_string_pretty(&state)
        .map_err(|error| format!("Could not serialize the workspace: {error}"))?;
    fs::write(&temporary, serialized)
        .map_err(|error| format!("Could not write the workspace: {error}"))?;
    fs::rename(&temporary, &path)
        .map_err(|error| format!("Could not finish saving the workspace: {error}"))
}

#[tauri::command]
fn set_watched_locations(
    app: AppHandle,
    state: State<WatchState>,
    locations: Vec<String>,
) -> Result<Vec<String>, String> {
    let mut unique_roots = Vec::new();
    let mut seen = HashSet::new();
    for location in locations {
        let Ok(root) = normalize_path(&location) else {
            continue;
        };
        if !root.is_dir() {
            continue;
        }
        if seen.insert(root.clone()) {
            unique_roots.push(root);
        }
    }

    let mut next_watchers = Vec::new();
    for root in &unique_roots {
        let app_handle = app.clone();
        let mut watcher =
            notify::recommended_watcher(move |event: Result<Event, notify::Error>| {
                if let Ok(event) = event {
                    let payload = FsChange {
                        paths: event
                            .paths
                            .iter()
                            .map(|path| path.to_string_lossy().to_string())
                            .collect(),
                        kind: format!("{:?}", event.kind),
                    };
                    let _ = app_handle.emit("filesystem-change", payload);
                }
            })
            .map_err(|error| format!("Could not start filesystem monitoring: {error}"))?;
        watcher
            .configure(Config::default())
            .map_err(|error| format!("Could not configure filesystem monitoring: {error}"))?;
        watcher
            .watch(root, RecursiveMode::Recursive)
            .map_err(|error| format!("Could not watch '{}': {error}", root.display()))?;
        next_watchers.push(watcher);
    }

    *state
        .roots
        .lock()
        .map_err(|_| "Filesystem monitoring is unavailable.".to_string())? = unique_roots.clone();
    *state
        .watchers
        .lock()
        .map_err(|_| "Filesystem monitoring is unavailable.".to_string())? = next_watchers;
    Ok(unique_roots
        .iter()
        .map(|root| root.to_string_lossy().to_string())
        .collect())
}

#[tauri::command]
fn read_image_data_url(path: String, state: State<WatchState>) -> Result<String, String> {
    let path = require_allowed(&path, &state)?;
    let mime = match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| extension.to_ascii_lowercase())
    {
        Some(extension) if extension == "png" => "image/png",
        Some(extension) if extension == "jpg" || extension == "jpeg" => "image/jpeg",
        Some(extension) if extension == "gif" => "image/gif",
        Some(extension) if extension == "webp" => "image/webp",
        Some(extension) if extension == "svg" => "image/svg+xml",
        _ => return Err("This image type is not supported in Preview.".to_string()),
    };
    let bytes = fs::read(&path).map_err(|error| format!("Could not read the image: {error}"))?;
    Ok(format!("data:{mime};base64,{}", BASE64.encode(bytes)))
}

#[tauri::command]
fn list_markdown_files(path: String, state: State<WatchState>) -> Result<Vec<FileEntry>, String> {
    let root = normalize_path(&path)?;
    if !root.is_dir() {
        return Err("O Local não está disponível.".to_string());
    }
    if !is_allowed(&root, &state) {
        return Err("This folder is not inside a registered Location.".to_string());
    }
    collect_files(&root)
}

#[tauri::command]
fn read_markdown_file(path: String, state: State<WatchState>) -> Result<FileContent, String> {
    let path = require_allowed(&path, &state)?;
    if !is_markdown(&path) {
        return Err("Only Markdown files are supported in this version.".to_string());
    }
    let content =
        fs::read_to_string(&path).map_err(|error| format!("Could not read the file: {error}"))?;
    let metadata =
        fs::metadata(&path).map_err(|error| format!("Could not read file metadata: {error}"))?;
    let line_ending = if content.contains("\r\n") {
        "CRLF"
    } else {
        "LF"
    }
    .to_string();
    Ok(FileContent {
        content,
        line_ending,
        modified_at_ms: timestamp_ms(&metadata),
    })
}

#[tauri::command]
fn inspect_okf_document(request: okf::InspectDocumentRequest) -> okf::OkfInspection {
    okf::inspect_document(request)
}

#[tauri::command]
async fn inspect_okf_bundle(
    path: String,
    state: State<'_, WatchState>,
) -> Result<okf::OkfBundleSnapshot, String> {
    let root = normalize_path(&path)?;
    if !root.is_dir() {
        return Err("The Location is not available.".to_string());
    }
    if !is_allowed(&root, &state) {
        return Err("This folder is not inside a registered Location.".to_string());
    }
    let files = collect_files(&root)?
        .into_iter()
        .map(|entry| okf::BundleFile {
            path: PathBuf::from(entry.path),
            relative_path: entry.relative_path,
        })
        .collect();
    tauri::async_runtime::spawn_blocking(move || okf::inspect_bundle(&root, files))
        .await
        .map_err(|error| format!("The OKF inspection task failed: {error}"))?
}

#[tauri::command]
async fn sync_location_index(
    request: index::SyncLocationRequest,
    watch_state: State<'_, WatchState>,
    knowledge: State<'_, knowledge::KnowledgeClient>,
) -> Result<index::IndexStatus, String> {
    let root = normalize_path(&request.root_path)?;
    if !root.is_dir() {
        return Err("The Location is not available.".to_string());
    }
    if !is_allowed(&root, &watch_state) {
        return Err("This folder is not inside a registered Location.".to_string());
    }
    knowledge.sync(request).await
}

#[tauri::command]
async fn get_location_index_status(
    location_id: String,
    knowledge: State<'_, knowledge::KnowledgeClient>,
) -> Result<index::IndexStatus, String> {
    knowledge.status(&location_id).await
}

#[tauri::command]
async fn search_location_index(
    request: index::SearchIndexRequest,
    knowledge: State<'_, knowledge::KnowledgeClient>,
) -> Result<Vec<index::SearchResult>, String> {
    knowledge.search(request).await
}

#[tauri::command]
async fn search_knowledge(
    request: index::KnowledgeSearchRequest,
    knowledge: State<'_, knowledge::KnowledgeClient>,
) -> Result<index::KnowledgeSearchResponse, String> {
    knowledge.search_knowledge(request).await
}

#[tauri::command]
async fn get_search_facets(
    request: index::SearchFacetsRequest,
    knowledge: State<'_, knowledge::KnowledgeClient>,
) -> Result<index::SearchFacets, String> {
    knowledge.search_facets(request).await
}

#[tauri::command]
async fn get_indexed_document(
    location_id: String,
    relative_path: String,
    knowledge: State<'_, knowledge::KnowledgeClient>,
) -> Result<Option<index::IndexedDocumentView>, String> {
    knowledge
        .get_document(&location_id, &relative_path, false)
        .await
}

#[tauri::command]
async fn get_related_documents(
    request: index::RelatedDocumentsRequest,
    knowledge: State<'_, knowledge::KnowledgeClient>,
) -> Result<index::RelatedDocumentsResponse, String> {
    knowledge.related_documents(request).await
}

#[tauri::command]
async fn build_context_pack(
    request: index::BuildContextPackRequest,
    knowledge: State<'_, knowledge::KnowledgeClient>,
) -> Result<index::ContextPackResponse, String> {
    knowledge.build_context_pack(request, false).await
}

#[tauri::command]
async fn delete_location_index(
    location_id: String,
    knowledge: State<'_, knowledge::KnowledgeClient>,
) -> Result<(), String> {
    knowledge.delete(&location_id).await
}

#[tauri::command]
fn get_mcp_configuration(
    location_id: String,
    knowledge: State<'_, knowledge::KnowledgeClient>,
) -> Result<String, String> {
    knowledge::mcp_configuration(knowledge.data_dir(), &location_id)
}

#[tauri::command]
fn write_markdown_file(request: WriteFileRequest, state: State<WatchState>) -> Result<(), String> {
    let path = require_allowed(&request.path, &state)?;
    if !is_markdown(&path) {
        return Err("Only Markdown files are supported in this version.".to_string());
    }
    let parent = path
        .parent()
        .ok_or_else(|| "The file does not have a parent folder.".to_string())?;
    let temporary = parent.join(format!(
        ".construct-{}.tmp",
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    fs::write(&temporary, request.content)
        .map_err(|error| format!("Could not write the temporary file: {error}"))?;
    fs::rename(&temporary, &path).map_err(|error| format!("Could not save the file: {error}"))
}

#[tauri::command]
fn get_git_info(path: String, state: State<WatchState>) -> Result<GitInfo, String> {
    let path = require_allowed(&path, &state)?;
    let Ok(root) = git_root(&path) else {
        return Ok(GitInfo {
            available: false,
            repo_root: None,
            status: None,
            has_head: false,
        });
    };
    Ok(GitInfo {
        available: true,
        repo_root: Some(root.to_string_lossy().to_string()),
        status: git_status(&path, &root),
        has_head: git_has_head(&root),
    })
}

#[tauri::command]
fn get_git_diff(
    path: String,
    content: Option<String>,
    state: State<WatchState>,
) -> Result<GitDiff, String> {
    let path = require_allowed(&path, &state)?;
    let root = match git_root(&path) {
        Ok(root) => root,
        Err(_) => {
            return Ok(GitDiff {
                available: false,
                diff: String::new(),
                message: Some("This file is not inside a Git repository.".to_string()),
            })
        }
    };
    let relative = path
        .strip_prefix(&root)
        .map_err(|error| format!("Could not calculate the Git path: {error}"))?
        .to_string_lossy()
        .to_string();
    if !git_has_head(&root) {
        let current = content.unwrap_or_else(|| fs::read_to_string(&path).unwrap_or_default());
        let diff = current
            .lines()
            .map(|line| format!("+{line}"))
            .collect::<Vec<_>>()
            .join("\n");
        return Ok(GitDiff {
            available: true,
            diff,
            message: Some(
                "This repository does not have a HEAD yet; the file is shown as an addition."
                    .to_string(),
            ),
        });
    }
    if let Some(buffer) = content {
        let head = Command::new("git")
            .current_dir(&root)
            .args(["show", &format!("HEAD:{relative}")])
            .output();
        let baseline = if let Ok(output) = head {
            if output.status.success() {
                String::from_utf8_lossy(&output.stdout).to_string()
            } else {
                String::new()
            }
        } else {
            String::new()
        };
        let temporary_dir = std::env::temp_dir();
        let stamp = Utc::now().timestamp_nanos_opt().unwrap_or_default();
        let before = temporary_dir.join(format!("construct-before-{stamp}.md"));
        let after = temporary_dir.join(format!("construct-after-{stamp}.md"));
        fs::write(&before, baseline)
            .map_err(|error| format!("Could not prepare the diff: {error}"))?;
        fs::write(&after, buffer)
            .map_err(|error| format!("Could not prepare the diff: {error}"))?;
        let output = Command::new("git")
            .args([
                "diff",
                "--no-index",
                "--no-color",
                "--",
                &before.to_string_lossy(),
                &after.to_string_lossy(),
            ])
            .output();
        let _ = fs::remove_file(&before);
        let _ = fs::remove_file(&after);
        let output = output.map_err(|error| format!("Could not generate the diff: {error}"))?;
        return Ok(GitDiff {
            available: true,
            diff: String::from_utf8_lossy(&output.stdout).to_string(),
            message: Some("The diff includes unsaved changes.".to_string()),
        });
    }
    let output = Command::new("git")
        .current_dir(&root)
        .args(["diff", "--no-color", "HEAD", "--", &relative])
        .output()
        .map_err(|error| format!("Could not generate the diff: {error}"))?;
    if output.status.success() {
        let diff = String::from_utf8_lossy(&output.stdout).to_string();
        if diff.is_empty() && git_status(&path, &root).is_some() {
            let current = fs::read_to_string(&path).unwrap_or_default();
            let additions = current
                .lines()
                .map(|line| format!("+{line}"))
                .collect::<Vec<_>>()
                .join("\n");
            return Ok(GitDiff {
                available: true,
                diff: additions,
                message: Some("Untracked file; shown as an addition.".to_string()),
            });
        }
        return Ok(GitDiff {
            available: true,
            diff,
            message: None,
        });
    }
    Ok(GitDiff {
        available: true,
        diff: String::new(),
        message: Some(String::from_utf8_lossy(&output.stderr).to_string()),
    })
}

#[tauri::command]
fn reveal_in_file_manager(path: String, state: State<WatchState>) -> Result<(), String> {
    let path = require_allowed(&path, &state)?;
    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg("-R").arg(&path).status();
    #[cfg(target_os = "windows")]
    let result = Command::new("explorer").arg("/select,").arg(&path).status();
    #[cfg(target_os = "linux")]
    let result = Command::new("xdg-open")
        .arg(path.parent().unwrap_or(&path))
        .status();
    result.map_err(|error| format!("Could not reveal the file: {error}"))?;
    Ok(())
}

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Only HTTP and HTTPS links can be opened externally.".to_string());
    }
    #[cfg(target_os = "macos")]
    let result = Command::new("open").arg(&url).status();
    #[cfg(target_os = "windows")]
    let result = Command::new("cmd").args(["/C", "start", "", &url]).status();
    #[cfg(target_os = "linux")]
    let result = Command::new("xdg-open").arg(&url).status();
    result.map_err(|error| format!("Could not open the link: {error}"))?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(WatchState::default())
        .setup(|app| {
            let data_directory = app.path().app_data_dir().map_err(std::io::Error::other)?;
            let knowledge =
                knowledge::KnowledgeClient::new(data_directory).map_err(std::io::Error::other)?;
            app.manage(knowledge);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_app_state,
            save_app_state,
            set_watched_locations,
            list_markdown_files,
            read_markdown_file,
            inspect_okf_document,
            inspect_okf_bundle,
            sync_location_index,
            get_location_index_status,
            search_location_index,
            search_knowledge,
            get_search_facets,
            get_indexed_document,
            get_related_documents,
            build_context_pack,
            delete_location_index,
            get_mcp_configuration,
            read_image_data_url,
            write_markdown_file,
            get_git_info,
            get_git_diff,
            reveal_in_file_manager,
            open_external_url,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao executar o Construct");
}

pub fn run_service_command(arguments: &[String]) -> Result<(), String> {
    knowledge::run_service_command(arguments)
}

pub fn run_mcp_command(arguments: &[String]) -> Result<(), String> {
    mcp::run_mcp_command(arguments)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_root() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "construct-test-{}",
            Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        fs::create_dir_all(&path).expect("create temporary directory");
        path
    }

    #[test]
    fn recognizes_supported_markdown_extensions() {
        assert!(is_markdown(Path::new("notes.md")));
        assert!(is_markdown(Path::new("notes.MARKDOWN")));
        assert!(!is_markdown(Path::new("notes.txt")));
        assert!(!is_markdown(Path::new("notes")));
    }

    #[test]
    fn discovery_includes_hidden_context_and_skips_generated_directories() {
        let root = temporary_root();
        fs::create_dir_all(root.join(".agents")).expect("create .agents");
        fs::create_dir_all(root.join("node_modules/package")).expect("create node_modules");
        fs::create_dir_all(root.join("target/doc")).expect("create target");
        fs::write(root.join("README.md"), "# Context").expect("create README");
        fs::write(root.join(".agents/memory.md"), "# Memory").expect("create memory");
        fs::write(root.join("node_modules/package/readme.md"), "# Dependency")
            .expect("create dependency");
        fs::write(root.join("target/doc/generated.md"), "# Generated")
            .expect("create generated file");

        let files = collect_files(&root).expect("discover files");
        let paths = files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, vec![".agents/memory.md", "README.md"]);

        fs::remove_dir_all(root).expect("remove temporary directory");
    }
}
