use crate::okf;
use blake3::Hasher;
use chrono::Utc;
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use surrealdb::{
    engine::local::{Db, SurrealKv},
    types::SurrealValue,
    Surreal,
};
use tokio::sync::Mutex;
use uuid::Uuid;
use walkdir::{DirEntry, WalkDir};

const INDEXER_VERSION: i64 = 1;
const MAX_DOCUMENT_BYTES: u64 = 8 * 1024 * 1024;
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

const SCHEMA: &str = r#"
DEFINE TABLE IF NOT EXISTS index_meta SCHEMALESS;
DEFINE TABLE IF NOT EXISTS document SCHEMALESS;
DEFINE ANALYZER IF NOT EXISTS construct TOKENIZERS blank, class, punct FILTERS lowercase, ascii;
DEFINE INDEX IF NOT EXISTS document_identity ON document FIELDS generation, relative_path UNIQUE;
DEFINE INDEX IF NOT EXISTS document_search ON document FIELDS search_text FULLTEXT ANALYZER construct BM25 HIGHLIGHTS;
"#;

#[derive(Clone)]
pub(crate) struct IndexService {
    base_dir: PathBuf,
    indexes: Arc<Mutex<HashMap<String, Arc<LocationIndex>>>>,
}

struct LocationIndex {
    db: Surreal<Db>,
    path: PathBuf,
    write_lock: Mutex<()>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SyncLocationRequest {
    pub(crate) location_id: String,
    pub(crate) root_path: String,
    pub(crate) display_name: String,
    #[serde(default)]
    pub(crate) okf_bundle: bool,
    #[serde(default)]
    pub(crate) rebuild: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchIndexRequest {
    pub(crate) location_id: String,
    pub(crate) query: String,
    #[serde(default = "default_search_limit")]
    pub(crate) limit: usize,
}

fn default_search_limit() -> usize {
    20
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IndexStatus {
    pub(crate) location_id: String,
    pub(crate) state: IndexState,
    pub(crate) active_generation: Option<i64>,
    pub(crate) building_generation: Option<i64>,
    pub(crate) discovered_documents: usize,
    pub(crate) indexed_documents: usize,
    pub(crate) failed_documents: usize,
    pub(crate) changed_documents: usize,
    pub(crate) removed_documents: usize,
    pub(crate) complete: bool,
    pub(crate) last_reconciled_at: Option<String>,
    pub(crate) storage_bytes: u64,
    pub(crate) error: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, SurrealValue)]
#[serde(rename_all = "camelCase")]
pub(crate) enum IndexState {
    NotIndexed,
    Indexing,
    Ready,
    Degraded,
    Failed,
}

#[derive(Clone, Debug, Serialize, Deserialize, SurrealValue)]
#[serde(rename_all = "camelCase")]
struct IndexMeta {
    location_id: String,
    root_path: String,
    display_name: String,
    indexer_version: i64,
    state: IndexState,
    active_generation: Option<i64>,
    building_generation: Option<i64>,
    discovered_documents: usize,
    indexed_documents: usize,
    failed_documents: usize,
    complete: bool,
    last_reconciled_at: Option<String>,
    error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, SurrealValue)]
struct StoredFingerprint {
    relative_path: String,
    continuity_id: String,
    content_hash: String,
    modified_at_ms: i64,
    size: u64,
}

#[derive(Clone, Debug, Serialize, SurrealValue)]
struct IndexedDocument {
    location_id: String,
    generation: i64,
    relative_path: String,
    continuity_id: String,
    content_hash: String,
    modified_at_ms: i64,
    size: u64,
    kind: String,
    title: String,
    description: Option<String>,
    r#type: Option<String>,
    tags: Vec<String>,
    headings: Vec<Heading>,
    frontmatter: Option<Value>,
    body: String,
    search_text: String,
    okf: Option<Value>,
    parse_error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, SurrealValue)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Heading {
    level: u8,
    text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, SurrealValue)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchResult {
    pub(crate) relative_path: String,
    pub(crate) title: String,
    pub(crate) description: Option<String>,
    pub(crate) r#type: Option<String>,
    pub(crate) tags: Vec<String>,
    pub(crate) score: f64,
    pub(crate) snippet: String,
    pub(crate) generation: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, SurrealValue)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IndexedDocumentView {
    pub(crate) relative_path: String,
    pub(crate) title: String,
    pub(crate) description: Option<String>,
    pub(crate) r#type: Option<String>,
    pub(crate) tags: Vec<String>,
    pub(crate) headings: Vec<Heading>,
    pub(crate) frontmatter: Option<Value>,
    pub(crate) body: String,
    pub(crate) generation: i64,
}

impl IndexStatus {
    fn not_indexed(location_id: &str) -> Self {
        Self {
            location_id: location_id.to_string(),
            state: IndexState::NotIndexed,
            active_generation: None,
            building_generation: None,
            discovered_documents: 0,
            indexed_documents: 0,
            failed_documents: 0,
            changed_documents: 0,
            removed_documents: 0,
            complete: false,
            last_reconciled_at: None,
            storage_bytes: 0,
            error: None,
        }
    }

    fn from_meta(meta: IndexMeta, storage_bytes: u64) -> Self {
        Self {
            location_id: meta.location_id,
            state: meta.state,
            active_generation: meta.active_generation,
            building_generation: meta.building_generation,
            discovered_documents: meta.discovered_documents,
            indexed_documents: meta.indexed_documents,
            failed_documents: meta.failed_documents,
            changed_documents: 0,
            removed_documents: 0,
            complete: meta.complete,
            last_reconciled_at: meta.last_reconciled_at,
            storage_bytes,
            error: meta.error,
        }
    }
}

impl IndexService {
    pub(crate) fn new(base_dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&base_dir)
            .map_err(|error| format!("Could not create the index directory: {error}"))?;
        Ok(Self {
            base_dir,
            indexes: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    fn storage_path(&self, location_id: &str) -> Result<PathBuf, String> {
        validate_location_id(location_id)?;
        let digest = blake3::hash(location_id.as_bytes()).to_hex();
        Ok(self.base_dir.join(digest.as_str()).join("surrealdb"))
    }

    async fn open(&self, location_id: &str) -> Result<Arc<LocationIndex>, String> {
        if let Some(index) = self.indexes.lock().await.get(location_id).cloned() {
            return Ok(index);
        }
        let path = self.storage_path(location_id)?;
        fs::create_dir_all(&path)
            .map_err(|error| format!("Could not create the Location index: {error}"))?;
        let db = Surreal::new::<SurrealKv>(path.clone())
            .await
            .map_err(|error| format!("Could not open the Location index: {error}"))?;
        db.use_ns("construct")
            .use_db("location")
            .await
            .map_err(|error| format!("Could not select the Location index: {error}"))?;
        db.query(SCHEMA)
            .await
            .map_err(|error| format!("Could not initialize the Location index: {error}"))?
            .check()
            .map_err(|error| format!("Could not initialize the Location index: {error}"))?;
        let index = Arc::new(LocationIndex {
            db,
            path,
            write_lock: Mutex::new(()),
        });
        let mut indexes = self.indexes.lock().await;
        Ok(indexes
            .entry(location_id.to_string())
            .or_insert_with(|| index.clone())
            .clone())
    }

    pub(crate) async fn status(&self, location_id: &str) -> Result<IndexStatus, String> {
        let path = self.storage_path(location_id)?;
        if !path.exists() {
            return Ok(IndexStatus::not_indexed(location_id));
        }
        let index = self.open(location_id).await?;
        let Some(meta) = read_meta(&index.db).await? else {
            return Ok(IndexStatus::not_indexed(location_id));
        };
        Ok(IndexStatus::from_meta(meta, directory_size(&index.path)))
    }

    pub(crate) async fn sync(
        &self,
        request: SyncLocationRequest,
        root: PathBuf,
    ) -> Result<IndexStatus, String> {
        let index = self.open(&request.location_id).await?;
        let _guard = index.write_lock.lock().await;
        let result = self.sync_locked(&index, &request, &root).await;
        if let Err(error) = &result {
            let previous = read_meta(&index.db).await.ok().flatten();
            let degraded = previous
                .as_ref()
                .and_then(|meta| meta.active_generation)
                .is_some();
            let meta = IndexMeta {
                location_id: request.location_id.clone(),
                root_path: root.to_string_lossy().to_string(),
                display_name: request.display_name.clone(),
                indexer_version: INDEXER_VERSION,
                state: if degraded {
                    IndexState::Degraded
                } else {
                    IndexState::Failed
                },
                active_generation: previous.as_ref().and_then(|meta| meta.active_generation),
                building_generation: None,
                discovered_documents: previous
                    .as_ref()
                    .map(|meta| meta.discovered_documents)
                    .unwrap_or_default(),
                indexed_documents: previous
                    .as_ref()
                    .map(|meta| meta.indexed_documents)
                    .unwrap_or_default(),
                failed_documents: previous
                    .as_ref()
                    .map(|meta| meta.failed_documents)
                    .unwrap_or_default(),
                complete: degraded,
                last_reconciled_at: previous.and_then(|meta| meta.last_reconciled_at),
                error: Some(error.clone()),
            };
            let _ = write_meta(&index.db, &meta).await;
        }
        result
    }

    async fn sync_locked(
        &self,
        index: &LocationIndex,
        request: &SyncLocationRequest,
        root: &Path,
    ) -> Result<IndexStatus, String> {
        let previous = read_meta(&index.db).await?;
        let full_build = request.rebuild
            || previous
                .as_ref()
                .and_then(|meta| meta.active_generation)
                .is_none()
            || previous
                .as_ref()
                .map(|meta| meta.indexer_version != INDEXER_VERSION)
                .unwrap_or(true);
        let active_generation = previous.as_ref().and_then(|meta| meta.active_generation);
        let generation = if full_build {
            active_generation.unwrap_or_default() + 1
        } else {
            active_generation.expect("an incremental sync has an active generation")
        };
        let entries = collect_markdown(root)?;
        let mut meta = IndexMeta {
            location_id: request.location_id.clone(),
            root_path: root.to_string_lossy().to_string(),
            display_name: request.display_name.clone(),
            indexer_version: INDEXER_VERSION,
            state: IndexState::Indexing,
            active_generation,
            building_generation: full_build.then_some(generation),
            discovered_documents: entries.len(),
            indexed_documents: previous
                .as_ref()
                .map(|value| value.indexed_documents)
                .unwrap_or_default(),
            failed_documents: 0,
            complete: active_generation.is_some(),
            last_reconciled_at: previous
                .as_ref()
                .and_then(|value| value.last_reconciled_at.clone()),
            error: None,
        };
        write_meta(&index.db, &meta).await?;

        let existing = if full_build {
            HashMap::new()
        } else {
            load_fingerprints(&index.db, generation).await?
        };
        let discovered_paths = entries
            .iter()
            .map(|entry| entry.relative_path.clone())
            .collect::<HashSet<_>>();
        let removed = existing
            .keys()
            .filter(|path| !discovered_paths.contains(*path))
            .cloned()
            .collect::<Vec<_>>();
        let mut changed = Vec::new();
        let mut failed = 0usize;

        for entry in &entries {
            if existing
                .get(&entry.relative_path)
                .map(|stored| {
                    stored.size == entry.size && stored.modified_at_ms == entry.modified_at_ms
                })
                .unwrap_or(false)
            {
                continue;
            }
            let bytes = match fs::read(&entry.path) {
                Ok(bytes) => bytes,
                Err(_) => {
                    failed += 1;
                    continue;
                }
            };
            let content_hash = blake3::hash(&bytes).to_hex().to_string();
            let content = match String::from_utf8(bytes) {
                Ok(content) => content,
                Err(_) => {
                    failed += 1;
                    continue;
                }
            };
            let continuity_id = existing
                .get(&entry.relative_path)
                .map(|stored| stored.continuity_id.clone())
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            changed.push(build_document(
                request,
                root,
                entry,
                generation,
                continuity_id,
                content_hash,
                &content,
            ));
        }

        write_documents(&index.db, generation, &changed, &removed).await?;
        let indexed_documents = count_documents(&index.db, generation).await?;
        meta.state = if failed > 0 {
            IndexState::Degraded
        } else {
            IndexState::Ready
        };
        meta.active_generation = Some(generation);
        meta.building_generation = None;
        meta.indexed_documents = indexed_documents;
        meta.failed_documents = failed;
        meta.complete = true;
        meta.last_reconciled_at = Some(Utc::now().to_rfc3339());
        write_meta(&index.db, &meta).await?;
        if full_build {
            delete_other_generations(&index.db, generation).await?;
        }
        let mut status = IndexStatus::from_meta(meta, directory_size(&index.path));
        status.changed_documents = changed.len();
        status.removed_documents = removed.len();
        Ok(status)
    }

    pub(crate) async fn search(
        &self,
        request: SearchIndexRequest,
    ) -> Result<Vec<SearchResult>, String> {
        let query = request.query.trim();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let index = self.open(&request.location_id).await?;
        let meta = read_meta(&index.db)
            .await?
            .ok_or_else(|| "This Location has not been indexed yet.".to_string())?;
        let generation = meta
            .active_generation
            .ok_or_else(|| "This Location has no complete index generation yet.".to_string())?;
        let limit = request.limit.clamp(1, 50) as i64;
        let mut response = index
            .db
            .query(
                r#"
SELECT relative_path, title, description, type, tags, generation,
       search::score(0) AS score,
       search::highlight('<mark>', '</mark>', 0) AS snippet
FROM document
WHERE generation = $generation AND search_text @0@ $query
ORDER BY score DESC
LIMIT $limit;
"#,
            )
            .bind(("generation", generation))
            .bind(("query", query.to_string()))
            .bind(("limit", limit))
            .await
            .map_err(|error| format!("Could not search the Location index: {error}"))?
            .check()
            .map_err(|error| format!("Could not search the Location index: {error}"))?;
        response
            .take(0)
            .map_err(|error| format!("Could not read the search results: {error}"))
    }

    pub(crate) async fn get_document(
        &self,
        location_id: &str,
        relative_path: &str,
    ) -> Result<Option<IndexedDocumentView>, String> {
        let index = self.open(location_id).await?;
        let Some(meta) = read_meta(&index.db).await? else {
            return Ok(None);
        };
        let Some(generation) = meta.active_generation else {
            return Ok(None);
        };
        let mut response = index
            .db
            .query(
                "SELECT relative_path, title, description, type, tags, headings, frontmatter, body, generation FROM document WHERE generation = $generation AND relative_path = $relative_path LIMIT 1;",
            )
            .bind(("generation", generation))
            .bind(("relative_path", normalize_relative_path(relative_path)))
            .await
            .map_err(|error| format!("Could not read the Location index: {error}"))?
            .check()
            .map_err(|error| format!("Could not read the Location index: {error}"))?;
        let documents: Vec<IndexedDocumentView> = response
            .take(0)
            .map_err(|error| format!("Could not read the indexed document: {error}"))?;
        Ok(documents.into_iter().next())
    }

    pub(crate) async fn delete(&self, location_id: &str) -> Result<(), String> {
        let path = self.storage_path(location_id)?;
        let removed = self.indexes.lock().await.remove(location_id);
        drop(removed);
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        if path.exists() {
            let location_dir = path
                .parent()
                .ok_or_else(|| "The index path is invalid.".to_string())?;
            fs::remove_dir_all(location_dir)
                .map_err(|error| format!("Could not delete the Location index: {error}"))?;
        }
        Ok(())
    }
}

#[derive(Clone)]
struct MarkdownEntry {
    path: PathBuf,
    relative_path: String,
    modified_at_ms: i64,
    size: u64,
}

fn validate_location_id(location_id: &str) -> Result<(), String> {
    if location_id.is_empty()
        || location_id.len() > 128
        || !location_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err("The Location identifier is invalid.".to_string());
    }
    Ok(())
}

fn normalize_relative_path(path: &str) -> String {
    path.replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>()
        .join("/")
}

fn is_markdown(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| matches!(extension.to_ascii_lowercase().as_str(), "md" | "markdown"))
        .unwrap_or(false)
}

fn is_ignored(entry: &DirEntry) -> bool {
    entry.file_type().is_dir()
        && entry
            .file_name()
            .to_str()
            .map(|name| {
                IGNORED_DIRECTORIES
                    .iter()
                    .any(|ignored| name.eq_ignore_ascii_case(ignored))
            })
            .unwrap_or(false)
}

fn collect_markdown(root: &Path) -> Result<Vec<MarkdownEntry>, String> {
    let mut entries = Vec::new();
    for entry in WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !is_ignored(entry))
    {
        let entry = entry.map_err(|error| format!("Could not scan the Location: {error}"))?;
        if !entry.file_type().is_file() || !is_markdown(entry.path()) {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|error| format!("Could not read Markdown metadata: {error}"))?;
        if metadata.len() > MAX_DOCUMENT_BYTES {
            continue;
        }
        let relative_path = entry
            .path()
            .strip_prefix(root)
            .map_err(|error| format!("Could not normalize a Markdown path: {error}"))?
            .to_string_lossy()
            .to_string();
        let modified_at_ms = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis() as i64)
            .unwrap_or_default();
        entries.push(MarkdownEntry {
            path: entry.path().to_path_buf(),
            relative_path: normalize_relative_path(&relative_path),
            modified_at_ms,
            size: metadata.len(),
        });
    }
    entries.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    Ok(entries)
}

fn headings(body: &str) -> Vec<Heading> {
    let mut output = Vec::new();
    let mut current: Option<(u8, String)> = None;
    for event in Parser::new_ext(body, Options::all()) {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                current = Some((level as u8, String::new()));
            }
            Event::Text(text) | Event::Code(text) => {
                if let Some((_, value)) = current.as_mut() {
                    value.push_str(&text);
                }
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some((level, text)) = current.take() {
                    let text = text.trim().to_string();
                    if !text.is_empty() {
                        output.push(Heading { level, text });
                    }
                }
            }
            _ => {}
        }
    }
    output
}

fn flatten_json(value: &Value, output: &mut Vec<String>) {
    match value {
        Value::Null => {}
        Value::Bool(value) => output.push(value.to_string()),
        Value::Number(value) => output.push(value.to_string()),
        Value::String(value) => output.push(value.clone()),
        Value::Array(values) => {
            for value in values {
                flatten_json(value, output);
            }
        }
        Value::Object(values) => {
            for (key, value) in values {
                output.push(key.clone());
                flatten_json(value, output);
            }
        }
    }
}

fn json_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn json_strings(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn build_document(
    request: &SyncLocationRequest,
    root: &Path,
    entry: &MarkdownEntry,
    generation: i64,
    continuity_id: String,
    content_hash: String,
    content: &str,
) -> IndexedDocument {
    let body = okf::visible_markdown_body(content).to_string();
    let headings = headings(&body);
    let inspection = okf::inspect_saved_document(
        content,
        &entry.relative_path,
        &entry.path,
        root,
        entry.relative_path.eq_ignore_ascii_case("index.md"),
    );
    let inspection_json = serde_json::to_value(inspection).unwrap_or(Value::Null);
    let metadata = inspection_json
        .get("metadata")
        .cloned()
        .unwrap_or(Value::Null);
    let title = json_string(&metadata, "title")
        .or_else(|| {
            headings
                .iter()
                .find(|heading| heading.level == 1)
                .map(|heading| heading.text.clone())
        })
        .unwrap_or_else(|| {
            entry
                .relative_path
                .rsplit('/')
                .next()
                .unwrap_or(&entry.relative_path)
                .to_string()
        });
    let description = json_string(&metadata, "description");
    let document_type = json_string(&metadata, "type");
    let tags = json_strings(&metadata, "tags");
    let frontmatter = metadata
        .get("raw")
        .cloned()
        .filter(|value| !value.is_null());
    let kind = json_string(&inspection_json, "kind").unwrap_or_else(|| "concept".to_string());
    let mut projection = vec![
        title.clone(),
        description.clone().unwrap_or_default(),
        document_type.clone().unwrap_or_default(),
        tags.join(" "),
        entry.relative_path.clone(),
        headings
            .iter()
            .map(|heading| heading.text.as_str())
            .collect::<Vec<_>>()
            .join(" "),
        body.clone(),
    ];
    if let Some(value) = frontmatter.as_ref() {
        flatten_json(value, &mut projection);
    }
    IndexedDocument {
        location_id: request.location_id.clone(),
        generation,
        relative_path: entry.relative_path.clone(),
        continuity_id,
        content_hash,
        modified_at_ms: entry.modified_at_ms,
        size: entry.size,
        kind,
        title,
        description,
        r#type: document_type,
        tags,
        headings,
        frontmatter,
        body,
        search_text: projection.join("\n"),
        okf: request.okf_bundle.then_some(inspection_json),
        parse_error: None,
    }
}

async fn read_meta(db: &Surreal<Db>) -> Result<Option<IndexMeta>, String> {
    db.select(("index_meta", "state"))
        .await
        .map_err(|error| format!("Could not read index metadata: {error}"))
}

async fn write_meta(db: &Surreal<Db>, meta: &IndexMeta) -> Result<(), String> {
    let _: Option<IndexMeta> = db
        .upsert(("index_meta", "state"))
        .content(meta.clone())
        .await
        .map_err(|error| format!("Could not update index metadata: {error}"))?;
    Ok(())
}

async fn load_fingerprints(
    db: &Surreal<Db>,
    generation: i64,
) -> Result<HashMap<String, StoredFingerprint>, String> {
    let mut response = db
        .query("SELECT relative_path, continuity_id, content_hash, modified_at_ms, size FROM document WHERE generation = $generation;")
        .bind(("generation", generation))
        .await
        .map_err(|error| format!("Could not read indexed fingerprints: {error}"))?
        .check()
        .map_err(|error| format!("Could not read indexed fingerprints: {error}"))?;
    let values: Vec<StoredFingerprint> = response
        .take(0)
        .map_err(|error| format!("Could not decode indexed fingerprints: {error}"))?;
    Ok(values
        .into_iter()
        .map(|value| (value.relative_path.clone(), value))
        .collect())
}

async fn write_documents(
    db: &Surreal<Db>,
    generation: i64,
    documents: &[IndexedDocument],
    removed: &[String],
) -> Result<(), String> {
    let mut query = String::from("BEGIN TRANSACTION;\n");
    for index in 0..documents.len() {
        query.push_str(&format!(
            "UPSERT type::record('document', $record_id_{index}) CONTENT $document_{index};\n"
        ));
    }
    query.push_str(
        "DELETE document WHERE generation = $generation AND relative_path IN $removed;\nCOMMIT TRANSACTION;",
    );
    let mut pending = db
        .query(query)
        .bind(("generation", generation))
        .bind(("removed", removed.to_vec()));
    for (position, document) in documents.iter().enumerate() {
        let mut hasher = Hasher::new();
        hasher.update(document.generation.to_string().as_bytes());
        hasher.update(b":");
        hasher.update(document.relative_path.as_bytes());
        let record_id = hasher.finalize().to_hex().to_string();
        pending = pending
            .bind((format!("record_id_{position}"), record_id))
            .bind((format!("document_{position}"), document.clone()));
    }
    pending
        .await
        .map_err(|error| format!("Could not update the Location index: {error}"))?
        .check()
        .map_err(|error| format!("Could not update the Location index: {error}"))?;
    Ok(())
}

async fn count_documents(db: &Surreal<Db>, generation: i64) -> Result<usize, String> {
    #[derive(Deserialize, SurrealValue)]
    struct CountRow {
        count: usize,
    }
    let mut response = db
        .query("SELECT count() AS count FROM document WHERE generation = $generation GROUP ALL;")
        .bind(("generation", generation))
        .await
        .map_err(|error| format!("Could not count indexed documents: {error}"))?
        .check()
        .map_err(|error| format!("Could not count indexed documents: {error}"))?;
    let rows: Vec<CountRow> = response
        .take(0)
        .map_err(|error| format!("Could not decode the indexed document count: {error}"))?;
    Ok(rows.first().map(|row| row.count).unwrap_or_default())
}

async fn delete_other_generations(db: &Surreal<Db>, active_generation: i64) -> Result<(), String> {
    db.query("DELETE document WHERE generation != $active_generation;")
        .bind(("active_generation", active_generation))
        .await
        .map_err(|error| format!("Could not clean obsolete index generations: {error}"))?
        .check()
        .map_err(|error| format!("Could not clean obsolete index generations: {error}"))?;
    Ok(())
}

fn directory_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .filter_map(|entry| entry.metadata().ok())
        .map(|metadata| metadata.len())
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_root(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("construct-index-{name}-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).expect("create temporary root");
        path
    }

    fn request(location_id: &str, root: &Path) -> SyncLocationRequest {
        SyncLocationRequest {
            location_id: location_id.to_string(),
            root_path: root.to_string_lossy().to_string(),
            display_name: location_id.to_string(),
            okf_bundle: false,
            rebuild: false,
        }
    }

    #[tokio::test]
    async fn isolates_locations_and_indexes_visible_markdown() {
        let data = temporary_root("data");
        let first = temporary_root("first");
        let second = temporary_root("second");
        fs::write(
            first.join("shared.md"),
            "---\ntitle: Alpha\ntags: [one]\n---\n<!-- construct-review:v1\n{\"comments\":[{\"id\":\"1\",\"quote\":\"nebula\",\"comment\":\"hidden\",\"createdAt\":\"2026-07-26T00:00:00Z\"}]}\n-->\n# Alpha\nVisible nebula",
        )
        .expect("write first");
        fs::write(second.join("shared.md"), "# Beta\nVisible orchard").expect("write second");
        let service = IndexService::new(data.join("indexes")).expect("create service");

        service
            .sync(request("first-location", &first), first.clone())
            .await
            .expect("index first");
        service
            .sync(request("second-location", &second), second.clone())
            .await
            .expect("index second");

        let alpha = service
            .search(SearchIndexRequest {
                location_id: "first-location".to_string(),
                query: "nebula".to_string(),
                limit: 20,
            })
            .await
            .expect("search first");
        let isolated = service
            .search(SearchIndexRequest {
                location_id: "second-location".to_string(),
                query: "nebula".to_string(),
                limit: 20,
            })
            .await
            .expect("search second");
        let hidden = service
            .search(SearchIndexRequest {
                location_id: "first-location".to_string(),
                query: "hidden".to_string(),
                limit: 20,
            })
            .await
            .expect("search review payload");

        assert_eq!(alpha.len(), 1);
        assert!(isolated.is_empty());
        assert!(hidden.is_empty());
        let indexed = service
            .get_document("first-location", "shared.md")
            .await
            .expect("get indexed document")
            .expect("document exists");
        assert_eq!(indexed.title, "Alpha");
        assert!(indexed.frontmatter.is_some());
        assert!(indexed.body.contains("Visible nebula"));
        assert!(!indexed.body.contains("construct-review"));
        assert_ne!(
            service.storage_path("first-location").unwrap(),
            service.storage_path("second-location").unwrap()
        );

        drop(service);
        fs::remove_dir_all(data).expect("remove data");
        fs::remove_dir_all(first).expect("remove first");
        fs::remove_dir_all(second).expect("remove second");
    }

    #[tokio::test]
    async fn updates_only_changed_documents_and_survives_restart() {
        let data = temporary_root("restart-data");
        let source = temporary_root("restart-source");
        fs::write(source.join("one.md"), "# One\nBefore").expect("write one");
        fs::write(source.join("two.md"), "# Two\nStable").expect("write two");
        let location_id = "restart-location";
        let service = IndexService::new(data.join("indexes")).expect("create service");
        let initial = service
            .sync(request(location_id, &source), source.clone())
            .await
            .expect("initial sync");
        assert_eq!(initial.changed_documents, 2);

        fs::write(source.join("one.md"), "# One\nAfter").expect("change one");
        let updated = service
            .sync(request(location_id, &source), source.clone())
            .await
            .expect("incremental sync");
        assert_eq!(updated.changed_documents, 1);
        assert_eq!(updated.removed_documents, 0);

        fs::remove_file(source.join("two.md")).expect("remove two");
        let removed = service
            .sync(request(location_id, &source), source.clone())
            .await
            .expect("delete sync");
        assert_eq!(removed.changed_documents, 0);
        assert_eq!(removed.removed_documents, 1);
        drop(service);
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let reopened = IndexService::new(data.join("indexes")).expect("reopen service");
        let status = reopened.status(location_id).await.expect("read status");
        assert_eq!(status.state, IndexState::Ready);
        assert_eq!(status.indexed_documents, 1);
        let result = reopened
            .search(SearchIndexRequest {
                location_id: location_id.to_string(),
                query: "After".to_string(),
                limit: 20,
            })
            .await
            .expect("search reopened index");
        assert_eq!(result.len(), 1);

        drop(reopened);
        fs::remove_dir_all(data).expect("remove data");
        fs::remove_dir_all(source).expect("remove source");
    }
}
