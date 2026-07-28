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

const INDEXER_VERSION: i64 = 4;
const MAX_DOCUMENT_BYTES: u64 = 8 * 1024 * 1024;
const LEXICAL_FALLBACK_BATCH_SIZE: usize = 200;
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
DEFINE TABLE IF NOT EXISTS document_link SCHEMALESS;
DEFINE TABLE IF NOT EXISTS document_activity_daily SCHEMALESS;
DEFINE ANALYZER IF NOT EXISTS construct TOKENIZERS blank, class, punct FILTERS lowercase, ascii;
DEFINE INDEX IF NOT EXISTS document_identity ON document FIELDS generation, relative_path UNIQUE;
DEFINE INDEX IF NOT EXISTS document_link_source ON document_link FIELDS generation, source_relative_path;
DEFINE INDEX IF NOT EXISTS document_link_target ON document_link FIELDS generation, target_relative_path;
DEFINE INDEX IF NOT EXISTS document_activity_day ON document_activity_daily FIELDS day;
DEFINE INDEX IF NOT EXISTS document_activity_path ON document_activity_daily FIELDS relative_path;
DEFINE INDEX IF NOT EXISTS document_search ON document FIELDS search_text FULLTEXT ANALYZER construct BM25 HIGHLIGHTS;
DEFINE INDEX IF NOT EXISTS document_title_search ON document FIELDS title FULLTEXT ANALYZER construct BM25;
DEFINE INDEX IF NOT EXISTS document_description_search ON document FIELDS description_text FULLTEXT ANALYZER construct BM25;
DEFINE INDEX IF NOT EXISTS document_type_search ON document FIELDS type_text FULLTEXT ANALYZER construct BM25;
DEFINE INDEX IF NOT EXISTS document_tags_search ON document FIELDS tags_text FULLTEXT ANALYZER construct BM25;
DEFINE INDEX IF NOT EXISTS document_headings_search ON document FIELDS headings_text FULLTEXT ANALYZER construct BM25;
DEFINE INDEX IF NOT EXISTS document_path_search ON document FIELDS relative_path_search FULLTEXT ANALYZER construct BM25;
DEFINE INDEX IF NOT EXISTS document_body_search ON document FIELDS body FULLTEXT ANALYZER construct BM25;
DEFINE INDEX IF NOT EXISTS document_metadata_search ON document FIELDS metadata_text FULLTEXT ANALYZER construct BM25;
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SyncLocationRequest {
    pub(crate) location_id: String,
    pub(crate) root_path: String,
    pub(crate) display_name: String,
    #[serde(default)]
    pub(crate) okf_bundle: bool,
    #[serde(default)]
    pub(crate) rebuild: bool,
    #[serde(default)]
    pub(crate) minimum_reconcile_interval_ms: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KnowledgeSearchFilters {
    #[serde(default)]
    pub(crate) types: Vec<String>,
    #[serde(default)]
    pub(crate) tags: Vec<String>,
    #[serde(default)]
    pub(crate) roles: Vec<String>,
    #[serde(default)]
    pub(crate) statuses: Vec<String>,
    #[serde(default)]
    pub(crate) trust: Vec<String>,
    #[serde(default)]
    pub(crate) freshness: Vec<String>,
    #[serde(default)]
    pub(crate) path_prefix: String,
    #[serde(default = "default_findings_filter")]
    pub(crate) findings: String,
}

fn default_findings_filter() -> String {
    "any".to_string()
}

impl Default for KnowledgeSearchFilters {
    fn default() -> Self {
        Self {
            types: Vec::new(),
            tags: Vec::new(),
            roles: Vec::new(),
            statuses: Vec::new(),
            trust: Vec::new(),
            freshness: Vec::new(),
            path_prefix: String::new(),
            findings: default_findings_filter(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KnowledgeSearchRequest {
    pub(crate) location_ids: Vec<String>,
    pub(crate) query: String,
    #[serde(default)]
    pub(crate) filters: KnowledgeSearchFilters,
    #[serde(default = "default_search_limit")]
    pub(crate) limit: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchFacetsRequest {
    pub(crate) location_ids: Vec<String>,
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
    description_text: String,
    r#type: Option<String>,
    type_text: String,
    tags: Vec<String>,
    tags_text: String,
    headings: Vec<Heading>,
    headings_text: String,
    frontmatter: Option<Value>,
    body: String,
    relative_path_search: String,
    metadata_text: String,
    search_text: String,
    okf: Option<Value>,
    parse_error: Option<String>,
    status: Option<String>,
    trust_tier: Option<String>,
    stale_after: Option<String>,
    finding_count: usize,
    links: Vec<IndexedLink>,
}

#[derive(Clone, Debug, Serialize, Deserialize, SurrealValue)]
struct IndexedLink {
    location_id: String,
    generation: i64,
    source_relative_path: String,
    target: String,
    target_relative_path: Option<String>,
    fragment: Option<String>,
    origin: String,
    field: Option<String>,
    status: String,
    start_line: Option<usize>,
    end_line: Option<usize>,
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KnowledgeSearchResult {
    pub(crate) location_id: String,
    pub(crate) relative_path: String,
    pub(crate) title: String,
    pub(crate) description: Option<String>,
    pub(crate) r#type: Option<String>,
    pub(crate) tags: Vec<String>,
    pub(crate) role: String,
    pub(crate) status: Option<String>,
    pub(crate) trust: Option<String>,
    pub(crate) freshness: String,
    pub(crate) stale_after: Option<String>,
    pub(crate) finding_count: usize,
    pub(crate) snippet: String,
    pub(crate) matched_fields: Vec<String>,
    pub(crate) match_reason: String,
    pub(crate) score: f64,
    pub(crate) rank_score: f64,
    pub(crate) generation: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KnowledgeSearchResponse {
    pub(crate) results: Vec<KnowledgeSearchResult>,
    pub(crate) unavailable_location_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FacetCount {
    pub(crate) value: String,
    pub(crate) count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchFacets {
    pub(crate) types: Vec<FacetCount>,
    pub(crate) tags: Vec<FacetCount>,
    pub(crate) roles: Vec<FacetCount>,
    pub(crate) statuses: Vec<FacetCount>,
    pub(crate) trust: Vec<FacetCount>,
    pub(crate) freshness: Vec<FacetCount>,
    pub(crate) unavailable_location_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, SurrealValue)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IndexedDocumentView {
    pub(crate) relative_path: String,
    pub(crate) title: String,
    pub(crate) description: Option<String>,
    pub(crate) r#type: Option<String>,
    pub(crate) tags: Vec<String>,
    pub(crate) role: String,
    pub(crate) headings: Vec<Heading>,
    pub(crate) frontmatter: Option<Value>,
    pub(crate) body: String,
    pub(crate) generation: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RelatedDocumentsRequest {
    pub(crate) location_id: String,
    pub(crate) relative_path: String,
    #[serde(default = "default_related_limit")]
    pub(crate) limit: usize,
}

fn default_related_limit() -> usize {
    20
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RelatedDocument {
    pub(crate) location_id: String,
    pub(crate) relative_path: String,
    pub(crate) title: String,
    pub(crate) r#type: Option<String>,
    pub(crate) tags: Vec<String>,
    pub(crate) role: String,
    pub(crate) direction: String,
    pub(crate) reason: String,
    pub(crate) fragment: Option<String>,
    pub(crate) generation: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RelatedDocumentsResponse {
    pub(crate) documents: Vec<RelatedDocument>,
    pub(crate) outgoing_count: usize,
    pub(crate) incoming_count: usize,
    pub(crate) omitted_count: usize,
    pub(crate) generation: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContextDocumentRef {
    pub(crate) location_id: String,
    pub(crate) relative_path: String,
    #[serde(default)]
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BuildContextPackRequest {
    #[serde(default)]
    pub(crate) query: String,
    pub(crate) documents: Vec<ContextDocumentRef>,
    #[serde(default = "default_context_characters")]
    pub(crate) max_characters: usize,
    #[serde(default = "default_context_documents")]
    pub(crate) max_documents: usize,
}

fn default_context_characters() -> usize {
    30_000
}

fn default_context_documents() -> usize {
    20
}

const MIN_CONTEXT_EXCERPT_CHARACTERS: usize = 160;
const HARD_MIN_CONTEXT_EXCERPT_CHARACTERS: usize = 80;
const CONTEXT_TRUNCATION_NOTE: &str = "\n\n[Content truncated by the character budget.]\n";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContextPackItem {
    pub(crate) location_id: String,
    pub(crate) relative_path: String,
    pub(crate) title: String,
    pub(crate) role: String,
    pub(crate) reason: String,
    pub(crate) content: String,
    pub(crate) characters: usize,
    pub(crate) truncated: bool,
    pub(crate) generation: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContextPackOmission {
    pub(crate) location_id: String,
    pub(crate) relative_path: String,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContextPackResponse {
    pub(crate) query: String,
    pub(crate) items: Vec<ContextPackItem>,
    pub(crate) omitted: Vec<ContextPackOmission>,
    pub(crate) total_characters: usize,
    pub(crate) max_characters: usize,
    pub(crate) truncated: bool,
    pub(crate) estimator: String,
    pub(crate) markdown: String,
}

struct PreparedContextDocument {
    location_id: String,
    relative_path: String,
    title: String,
    role: String,
    reason: String,
    body: String,
    generation: i64,
    prefix: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocationActivityRequest {
    pub(crate) location_id: String,
    #[serde(default = "default_activity_days")]
    pub(crate) days: usize,
    #[serde(default = "default_activity_limit")]
    pub(crate) limit: usize,
    #[serde(default)]
    pub(crate) path_prefix: String,
}

fn default_activity_days() -> usize {
    15
}

fn default_activity_limit() -> usize {
    20
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, SurrealValue)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DocumentActivity {
    pub(crate) relative_path: String,
    pub(crate) changed_count: usize,
    pub(crate) served_count: usize,
    pub(crate) context_count: usize,
    pub(crate) created_count: usize,
    pub(crate) removed_count: usize,
    pub(crate) last_changed_at: Option<String>,
    pub(crate) last_served_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocationActivityResponse {
    pub(crate) location_id: String,
    pub(crate) window_days: usize,
    pub(crate) generated_at: String,
    pub(crate) documents: Vec<DocumentActivity>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LogEntry {
    pub(crate) relative_path: String,
    pub(crate) scope: String,
    pub(crate) date: Option<String>,
    pub(crate) summary: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocationOverview {
    pub(crate) location_id: String,
    pub(crate) status: IndexStatus,
    pub(crate) types: Vec<FacetCount>,
    pub(crate) tags: Vec<FacetCount>,
    pub(crate) roles: Vec<FacetCount>,
    pub(crate) statuses: Vec<FacetCount>,
    pub(crate) trust: Vec<FacetCount>,
    pub(crate) resolved_links: usize,
    pub(crate) unresolved_links: usize,
    pub(crate) findings: usize,
    pub(crate) recent_logs: Vec<LogEntry>,
    pub(crate) activity: LocationActivityResponse,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ActivityKind {
    Served,
    Context,
}

#[derive(Clone, Debug, Serialize, Deserialize, SurrealValue)]
struct ActivityDailyRow {
    relative_path: String,
    day: String,
    changed_count: usize,
    served_count: usize,
    context_count: usize,
    created_count: usize,
    removed_count: usize,
    last_changed_at: Option<String>,
    last_served_at: Option<String>,
}

#[derive(Clone, Debug, Deserialize, SurrealValue)]
struct LocalSearchRow {
    relative_path: String,
    title: String,
    description: Option<String>,
    r#type: Option<String>,
    tags: Vec<String>,
    kind: String,
    status: Option<String>,
    trust_tier: Option<String>,
    stale_after: Option<String>,
    finding_count: usize,
    body: String,
    headings_text: String,
    metadata_text: String,
    generation: i64,
    title_score: f64,
    description_score: f64,
    type_score: f64,
    tags_score: f64,
    headings_score: f64,
    path_score: f64,
    body_score: f64,
    metadata_score: f64,
}

#[derive(Clone, Debug, Deserialize, SurrealValue)]
struct FacetRow {
    r#type: Option<String>,
    tags: Vec<String>,
    kind: String,
    status: Option<String>,
    trust_tier: Option<String>,
    stale_after: Option<String>,
    finding_count: usize,
}

#[derive(Clone, Debug, Deserialize, SurrealValue)]
struct StoredLink {
    source_relative_path: String,
    target_relative_path: Option<String>,
    fragment: Option<String>,
}

#[derive(Clone, Debug, Deserialize, SurrealValue)]
struct RelatedDocumentRow {
    relative_path: String,
    title: String,
    r#type: Option<String>,
    tags: Vec<String>,
    kind: String,
    generation: i64,
}

#[derive(Clone, Debug, Deserialize, SurrealValue)]
struct LinkStatusRow {
    status: String,
}

#[derive(Clone, Debug, Deserialize, SurrealValue)]
struct LogDocumentRow {
    relative_path: String,
    body: String,
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
        if !request.rebuild && request.minimum_reconcile_interval_ms > 0 {
            if let Some(meta) = read_meta(&index.db).await? {
                if can_skip_reconciliation(&meta, request.minimum_reconcile_interval_ms) {
                    return Ok(IndexStatus::from_meta(meta, directory_size(&index.path)));
                }
            }
        }
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
        if full_build {
            write_meta(&index.db, &meta).await?;
        }

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
        let build_context = DocumentBuildContext {
            request,
            root,
            discovered_paths: &discovered_paths,
            generation,
        };

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
                &build_context,
                entry,
                continuity_id,
                content_hash,
                &content,
            ));
        }

        let has_document_changes = full_build || !changed.is_empty() || !removed.is_empty();
        let indexed_documents = if has_document_changes {
            write_documents(&index.db, generation, &changed, &removed).await?;
            refresh_link_resolutions(&index.db, generation, &discovered_paths).await?;
            count_documents(&index.db, generation).await?
        } else {
            previous
                .as_ref()
                .map(|value| value.indexed_documents)
                .unwrap_or_default()
        };
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
        } else if has_document_changes {
            let now = Utc::now().to_rfc3339();
            for document in &changed {
                let created = !existing.contains_key(&document.relative_path);
                record_daily_activity(
                    &index.db,
                    &document.relative_path,
                    if created { "created" } else { "changed" },
                    &now,
                )
                .await?;
            }
            for relative_path in &removed {
                record_daily_activity(&index.db, relative_path, "removed", &now).await?;
            }
            cleanup_activity(&index.db).await?;
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
        let fulltext_results = index
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
            .map_err(|error| format!("Could not search the Location index: {error}"))
            .and_then(|response| {
                response
                    .check()
                    .map_err(|error| format!("Could not search the Location index: {error}"))
            })
            .and_then(|mut response| {
                response
                    .take::<Vec<SearchResult>>(0)
                    .map_err(|error| format!("Could not read the search results: {error}"))
            });
        match fulltext_results {
            Ok(results) if !results.is_empty() => Ok(results),
            _ => self
                .search_location_knowledge(
                    &request.location_id,
                    query,
                    &KnowledgeSearchFilters::default(),
                    limit as usize,
                )
                .await
                .map(|results| {
                    results
                        .into_iter()
                        .map(|result| SearchResult {
                            relative_path: result.relative_path,
                            title: result.title,
                            description: result.description,
                            r#type: result.r#type,
                            tags: result.tags,
                            score: result.score,
                            snippet: result.snippet,
                            generation: result.generation,
                        })
                        .collect()
                }),
        }
    }

    pub(crate) async fn search_knowledge(
        &self,
        request: KnowledgeSearchRequest,
    ) -> Result<KnowledgeSearchResponse, String> {
        validate_knowledge_search(&request)?;
        let query = request.query.trim().to_string();
        if query.is_empty() {
            return Ok(KnowledgeSearchResponse {
                results: Vec::new(),
                unavailable_location_ids: Vec::new(),
            });
        }
        let per_location_limit = request.limit.clamp(1, 100).saturating_mul(3).clamp(30, 200);
        let mut ranked = Vec::new();
        let mut unavailable = Vec::new();

        for (location_order, location_id) in request.location_ids.iter().enumerate() {
            match self
                .search_location_knowledge(
                    location_id,
                    &query,
                    &request.filters,
                    per_location_limit,
                )
                .await
            {
                Ok(results) => {
                    for (position, mut result) in results.into_iter().enumerate() {
                        result.rank_score = 1.0 / (60.0 + (position + 1) as f64);
                        ranked.push((location_order, position, result));
                    }
                }
                Err(_) => unavailable.push(location_id.clone()),
            }
        }

        ranked.sort_by(|left, right| {
            right
                .2
                .rank_score
                .total_cmp(&left.2.rank_score)
                .then_with(|| right.2.score.total_cmp(&left.2.score))
                .then_with(|| left.0.cmp(&right.0))
                .then_with(|| left.1.cmp(&right.1))
                .then_with(|| left.2.relative_path.cmp(&right.2.relative_path))
        });
        let results = ranked
            .into_iter()
            .map(|(_, _, result)| result)
            .take(request.limit.clamp(1, 100))
            .collect();
        Ok(KnowledgeSearchResponse {
            results,
            unavailable_location_ids: unavailable,
        })
    }

    async fn search_location_knowledge(
        &self,
        location_id: &str,
        query: &str,
        filters: &KnowledgeSearchFilters,
        limit: usize,
    ) -> Result<Vec<KnowledgeSearchResult>, String> {
        let index = self.open(location_id).await?;
        let meta = read_meta(&index.db)
            .await?
            .ok_or_else(|| "This Location has not been indexed yet.".to_string())?;
        let generation = meta
            .active_generation
            .ok_or_else(|| "This Location has no complete index generation yet.".to_string())?;
        let today = Utc::now().date_naive().format("%Y-%m-%d").to_string();
        let fulltext_query = exact_phrase(query).unwrap_or(query).to_string();
        let fulltext_rows = index
            .db
            .query(
                r#"
SELECT relative_path, title, description, type, tags, kind, status, trust_tier,
       stale_after, finding_count, body, headings_text, metadata_text, generation,
       search::score(0) AS title_score,
       search::score(1) AS description_score,
       search::score(2) AS type_score,
       search::score(3) AS tags_score,
       search::score(4) AS headings_score,
       search::score(5) AS path_score,
       search::score(6) AS body_score,
       search::score(7) AS metadata_score,
       (
         search::score(0) * 8 +
         search::score(1) * 5 +
         search::score(2) * 4 +
         search::score(3) * 4 +
         search::score(4) * 3 +
         search::score(5) * 2 +
         search::score(6) +
         search::score(7)
       ) AS weighted_score
FROM document
WHERE generation = $generation
  AND (
    title @0@ $query OR
    description_text @1@ $query OR
    type_text @2@ $query OR
    tags_text @3@ $query OR
    headings_text @4@ $query OR
    relative_path_search @5@ $query OR
    body @6@ $query OR
    metadata_text @7@ $query
  )
  AND (array::len($types) = 0 OR type IN $types)
  AND (array::len($tags) = 0 OR array::len(array::intersect(tags, $tags)) > 0)
  AND (array::len($roles) = 0 OR kind IN $roles)
  AND (array::len($statuses) = 0 OR status IN $statuses)
  AND (array::len($trust) = 0 OR trust_tier IN $trust)
  AND ($path_prefix = '' OR string::starts_with(relative_path, $path_prefix))
  AND (
    $findings = 'any' OR
    ($findings = 'with' AND finding_count > 0) OR
    ($findings = 'without' AND finding_count = 0)
  )
  AND (
    array::len($freshness) = 0 OR
    ('stale' IN $freshness AND stale_after != none AND stale_after <= $today) OR
    ('current' IN $freshness AND stale_after != none AND stale_after > $today) OR
    ('unspecified' IN $freshness AND stale_after = none)
  )
ORDER BY weighted_score DESC, relative_path ASC
LIMIT $limit;
"#,
            )
            .bind(("generation", generation))
            .bind(("query", fulltext_query))
            .bind(("types", filters.types.clone()))
            .bind(("tags", filters.tags.clone()))
            .bind(("roles", filters.roles.clone()))
            .bind(("statuses", filters.statuses.clone()))
            .bind(("trust", filters.trust.clone()))
            .bind(("freshness", filters.freshness.clone()))
            .bind(("path_prefix", normalize_relative_path(&filters.path_prefix)))
            .bind(("findings", filters.findings.clone()))
            .bind(("today", today.clone()))
            .bind(("limit", limit as i64))
            .await
            .map_err(|_| "Could not search this Location. Try a simpler query.".to_string())
            .and_then(|response| {
                response
                    .check()
                    .map_err(|_| "Could not search this Location. Try a simpler query.".to_string())
            })
            .and_then(|mut response| {
                response
                    .take::<Vec<LocalSearchRow>>(0)
                    .map_err(|_| "Could not read the local search results.".to_string())
            });
        let rows = match fulltext_rows {
            Ok(rows) if !rows.is_empty() => rows,
            _ => {
                return lexical_fallback_search(
                    &index.db,
                    location_id,
                    generation,
                    query,
                    filters,
                    &today,
                    limit,
                )
                .await;
            }
        };
        let mut results = rows
            .into_iter()
            .filter(|row| {
                exact_phrase(query)
                    .map(|phrase| row_contains_phrase(row, phrase))
                    .unwrap_or(true)
            })
            .map(|row| knowledge_result(location_id, query, row))
            .collect::<Vec<_>>();
        results.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.relative_path.cmp(&right.relative_path))
        });
        Ok(results)
    }

    pub(crate) async fn search_facets(
        &self,
        request: SearchFacetsRequest,
    ) -> Result<SearchFacets, String> {
        let mut type_counts = HashMap::new();
        let mut tag_counts = HashMap::new();
        let mut role_counts = HashMap::new();
        let mut status_counts = HashMap::new();
        let mut trust_counts = HashMap::new();
        let mut freshness_counts = HashMap::new();
        let mut unavailable = Vec::new();

        for location_id in &request.location_ids {
            let rows = match self.facet_rows(location_id).await {
                Ok(rows) => rows,
                Err(_) => {
                    unavailable.push(location_id.clone());
                    continue;
                }
            };
            for row in rows {
                if let Some(value) = row.r#type {
                    increment_count(&mut type_counts, value);
                }
                for value in row.tags {
                    increment_count(&mut tag_counts, value);
                }
                increment_count(&mut role_counts, row.kind);
                if let Some(value) = row.status {
                    increment_count(&mut status_counts, value);
                }
                if let Some(value) = row.trust_tier {
                    increment_count(&mut trust_counts, value);
                }
                increment_count(
                    &mut freshness_counts,
                    freshness_for(row.stale_after.as_deref()),
                );
            }
        }

        Ok(SearchFacets {
            types: sorted_counts(type_counts),
            tags: sorted_counts(tag_counts),
            roles: sorted_counts(role_counts),
            statuses: sorted_counts(status_counts),
            trust: sorted_counts(trust_counts),
            freshness: sorted_counts(freshness_counts),
            unavailable_location_ids: unavailable,
        })
    }

    async fn facet_rows(&self, location_id: &str) -> Result<Vec<FacetRow>, String> {
        let index = self.open(location_id).await?;
        let meta = read_meta(&index.db)
            .await?
            .ok_or_else(|| "This Location has not been indexed yet.".to_string())?;
        let generation = meta
            .active_generation
            .ok_or_else(|| "This Location has no complete index generation yet.".to_string())?;
        let mut response = index
            .db
            .query(
                "SELECT type, tags, kind, status, trust_tier, stale_after, finding_count FROM document WHERE generation = $generation;",
            )
            .bind(("generation", generation))
            .await
            .map_err(|_| "Could not read search facets for this Location.".to_string())?
            .check()
            .map_err(|_| "Could not read search facets for this Location.".to_string())?;
        response
            .take(0)
            .map_err(|_| "Could not decode search facets for this Location.".to_string())
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
                "SELECT relative_path, title, description, type, tags, kind AS role, headings, frontmatter, body, generation FROM document WHERE generation = $generation AND relative_path = $relative_path LIMIT 1;",
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

    pub(crate) async fn related_documents(
        &self,
        request: RelatedDocumentsRequest,
    ) -> Result<RelatedDocumentsResponse, String> {
        validate_location_id(&request.location_id)?;
        let relative_path = normalize_relative_path(&request.relative_path);
        if relative_path.is_empty() {
            return Err("Choose a document before loading related knowledge.".to_string());
        }
        let index = self.open(&request.location_id).await?;
        let meta = read_meta(&index.db)
            .await?
            .ok_or_else(|| "This Location has not been indexed yet.".to_string())?;
        let generation = meta
            .active_generation
            .ok_or_else(|| "This Location has no complete index generation yet.".to_string())?;
        let mut link_response = index
            .db
            .query(
                r#"
SELECT source_relative_path, target_relative_path, fragment
FROM document_link
WHERE generation = $generation
  AND status = 'resolved'
  AND (
    source_relative_path = $relative_path OR
    target_relative_path = $relative_path
  );
"#,
            )
            .bind(("generation", generation))
            .bind(("relative_path", relative_path.clone()))
            .await
            .map_err(|error| format!("Could not read related Markdown links: {error}"))?
            .check()
            .map_err(|error| format!("Could not read related Markdown links: {error}"))?;
        let links: Vec<StoredLink> = link_response
            .take(0)
            .map_err(|error| format!("Could not decode related Markdown links: {error}"))?;

        #[derive(Default)]
        struct Relationship {
            outgoing: bool,
            incoming: bool,
            fragment: Option<String>,
        }

        let mut relationships: HashMap<String, Relationship> = HashMap::new();
        for link in links {
            if link.source_relative_path == relative_path {
                if let Some(target) = link.target_relative_path {
                    if target != relative_path {
                        let relationship = relationships.entry(target).or_default();
                        relationship.outgoing = true;
                        if relationship.fragment.is_none() {
                            relationship.fragment = link.fragment;
                        }
                    }
                }
            } else if link.target_relative_path.as_deref() == Some(relative_path.as_str()) {
                let relationship = relationships.entry(link.source_relative_path).or_default();
                relationship.incoming = true;
            }
        }

        if relationships.is_empty() {
            return Ok(RelatedDocumentsResponse {
                documents: Vec::new(),
                outgoing_count: 0,
                incoming_count: 0,
                omitted_count: 0,
                generation,
            });
        }

        let related_paths = relationships.keys().cloned().collect::<Vec<_>>();
        let mut document_response = index
            .db
            .query(
                r#"
SELECT relative_path, title, type, tags, kind, generation
FROM document
WHERE generation = $generation AND relative_path IN $relative_paths;
"#,
            )
            .bind(("generation", generation))
            .bind(("relative_paths", related_paths))
            .await
            .map_err(|error| format!("Could not read related documents: {error}"))?
            .check()
            .map_err(|error| format!("Could not read related documents: {error}"))?;
        let rows: Vec<RelatedDocumentRow> = document_response
            .take(0)
            .map_err(|error| format!("Could not decode related documents: {error}"))?;

        let mut documents = rows
            .into_iter()
            .filter_map(|row| {
                let relationship = relationships.remove(&row.relative_path)?;
                let (direction, reason) = match (relationship.outgoing, relationship.incoming) {
                    (true, true) => (
                        "mutual".to_string(),
                        format!("Linked in both directions with {relative_path}"),
                    ),
                    (true, false) => (
                        "outgoing".to_string(),
                        format!("Linked from {relative_path}"),
                    ),
                    (false, true) => ("incoming".to_string(), format!("Links to {relative_path}")),
                    (false, false) => return None,
                };
                Some(RelatedDocument {
                    location_id: request.location_id.clone(),
                    relative_path: row.relative_path,
                    title: row.title,
                    r#type: row.r#type,
                    tags: row.tags,
                    role: row.kind,
                    direction,
                    reason,
                    fragment: relationship.fragment,
                    generation: row.generation,
                })
            })
            .collect::<Vec<_>>();
        documents.sort_by(|left, right| {
            related_direction_rank(&left.direction)
                .cmp(&related_direction_rank(&right.direction))
                .then_with(|| left.title.to_lowercase().cmp(&right.title.to_lowercase()))
                .then_with(|| left.relative_path.cmp(&right.relative_path))
        });
        let outgoing_count = documents
            .iter()
            .filter(|document| matches!(document.direction.as_str(), "outgoing" | "mutual"))
            .count();
        let incoming_count = documents
            .iter()
            .filter(|document| matches!(document.direction.as_str(), "incoming" | "mutual"))
            .count();
        let limit = request.limit.clamp(1, 50);
        let omitted_count = documents.len().saturating_sub(limit);
        documents.truncate(limit);
        Ok(RelatedDocumentsResponse {
            documents,
            outgoing_count,
            incoming_count,
            omitted_count,
            generation,
        })
    }

    pub(crate) async fn build_context_pack(
        &self,
        request: BuildContextPackRequest,
    ) -> Result<ContextPackResponse, String> {
        if request.query.len() > 1_000 {
            return Err("The context query is too long.".to_string());
        }
        if request.documents.is_empty() {
            return Err("Select at least one document for the context pack.".to_string());
        }
        if request.documents.len() > 100 {
            return Err("Too many documents were selected for one context pack.".to_string());
        }
        let max_characters = request.max_characters.clamp(1_000, 200_000);
        let max_documents = request.max_documents.clamp(1, 20);
        let query = request.query.trim().to_string();
        let mut markdown = context_pack_header(&query, max_characters);
        let content_limit = max_characters.saturating_sub(200);
        let mut items = Vec::new();
        let mut omitted = Vec::new();
        let mut seen = HashSet::new();
        let mut prepared = Vec::new();

        for reference in request.documents {
            validate_location_id(&reference.location_id)?;
            let relative_path = normalize_relative_path(&reference.relative_path);
            if relative_path.is_empty()
                || !seen.insert(format!("{}:{relative_path}", reference.location_id))
            {
                continue;
            }
            if prepared.len() >= max_documents {
                omitted.push(ContextPackOmission {
                    location_id: reference.location_id,
                    relative_path,
                    reason: "Document limit reached".to_string(),
                });
                continue;
            }
            let document = match self
                .get_document(&reference.location_id, &relative_path)
                .await
            {
                Ok(Some(document)) => document,
                Ok(None) => {
                    omitted.push(ContextPackOmission {
                        location_id: reference.location_id,
                        relative_path,
                        reason: "Document is not available in the active index".to_string(),
                    });
                    continue;
                }
                Err(_) => {
                    omitted.push(ContextPackOmission {
                        location_id: reference.location_id,
                        relative_path,
                        reason: "Location index is unavailable".to_string(),
                    });
                    continue;
                }
            };
            let reason = if reference.reason.trim().is_empty() {
                "Selected manually".to_string()
            } else {
                reference.reason.trim().to_string()
            };
            let prefix = context_document_prefix(
                &document.title,
                &reference.location_id,
                &relative_path,
                &document.role,
                &reason,
            );
            prepared.push(PreparedContextDocument {
                location_id: reference.location_id,
                relative_path,
                title: document.title,
                role: document.role,
                reason,
                body: document.body.trim().to_string(),
                generation: document.generation,
                prefix,
            });
        }

        let available = content_limit.saturating_sub(markdown.chars().count());
        let mut budget_omissions = Vec::new();
        while minimum_context_cost(&prepared) > available {
            let Some(document) = prepared.pop() else {
                break;
            };
            budget_omissions.push(ContextPackOmission {
                location_id: document.location_id,
                relative_path: document.relative_path,
                reason: "Character budget reached".to_string(),
            });
        }
        budget_omissions.reverse();
        omitted.extend(budget_omissions);

        let body_lengths = prepared
            .iter()
            .map(|document| document.body.chars().count())
            .collect::<Vec<_>>();
        let prefix_characters = prepared
            .iter()
            .map(|document| document.prefix.chars().count())
            .sum::<usize>();
        let mut truncation_reserved = body_lengths
            .iter()
            .map(|length| *length > HARD_MIN_CONTEXT_EXCERPT_CHARACTERS)
            .collect::<Vec<_>>();
        let suffix_characters = truncation_reserved
            .iter()
            .map(|truncated| {
                if *truncated {
                    CONTEXT_TRUNCATION_NOTE.chars().count()
                } else {
                    1
                }
            })
            .sum::<usize>();
        let body_budget =
            available.saturating_sub(prefix_characters.saturating_add(suffix_characters));
        let mut allocations = allocate_context_body_characters(&body_lengths, body_budget);

        loop {
            let mut reclaimed = 0;
            for (index, reserved) in truncation_reserved.iter_mut().enumerate() {
                if *reserved && allocations[index] >= body_lengths[index] {
                    *reserved = false;
                    reclaimed += CONTEXT_TRUNCATION_NOTE.chars().count().saturating_sub(1);
                }
            }
            if reclaimed == 0 {
                break;
            }
            distribute_context_budget_proportionally(&body_lengths, &mut allocations, reclaimed);
        }

        for (document, body_characters) in prepared.into_iter().zip(allocations) {
            let truncated = body_characters < document.body.chars().count();
            let content = truncate_characters(&document.body, body_characters);
            let suffix = if truncated {
                CONTEXT_TRUNCATION_NOTE
            } else {
                "\n"
            };
            let block = format!("{}{content}{suffix}", document.prefix);
            let characters = block.chars().count();
            markdown.push_str(&block);
            items.push(ContextPackItem {
                location_id: document.location_id,
                relative_path: document.relative_path,
                title: document.title,
                role: document.role,
                reason: document.reason,
                content,
                characters,
                truncated,
                generation: document.generation,
            });
        }

        let truncated_items = items.iter().filter(|item| item.truncated).count();
        if !omitted.is_empty() || truncated_items > 0 {
            markdown.push_str(&format!(
                "\n---\nContext pack truncated: {} document(s) omitted; {} included document(s) shortened.\n",
                omitted.len(),
                truncated_items
            ));
        }
        let total_characters = markdown.chars().count();
        let truncated = !omitted.is_empty() || truncated_items > 0;
        debug_assert!(total_characters <= max_characters);
        Ok(ContextPackResponse {
            query,
            items,
            omitted,
            total_characters,
            max_characters,
            truncated,
            estimator: "characters".to_string(),
            markdown,
        })
    }

    pub(crate) async fn record_document_activity(
        &self,
        location_id: &str,
        relative_path: &str,
        kind: ActivityKind,
    ) -> Result<(), String> {
        validate_location_id(location_id)?;
        let relative_path = normalize_relative_path(relative_path);
        if relative_path.is_empty() {
            return Err("The activity document path is invalid.".to_string());
        }
        let index = self.open(location_id).await?;
        let _guard = index.write_lock.lock().await;
        let now = Utc::now().to_rfc3339();
        record_daily_activity(
            &index.db,
            &relative_path,
            match kind {
                ActivityKind::Served => "served",
                ActivityKind::Context => "context",
            },
            &now,
        )
        .await?;
        cleanup_activity(&index.db).await
    }

    pub(crate) async fn location_activity(
        &self,
        request: LocationActivityRequest,
    ) -> Result<LocationActivityResponse, String> {
        validate_location_id(&request.location_id)?;
        let days = request.days.clamp(1, 15);
        let limit = request.limit.clamp(1, 50);
        let cutoff = (Utc::now().date_naive() - chrono::Duration::days((days - 1) as i64))
            .format("%Y-%m-%d")
            .to_string();
        let prefix = normalize_relative_path(&request.path_prefix);
        let index = self.open(&request.location_id).await?;
        let mut response = index
            .db
            .query(
                r#"
SELECT relative_path, day, changed_count, served_count, context_count,
       created_count, removed_count, last_changed_at, last_served_at
FROM document_activity_daily
WHERE day >= $cutoff
  AND ($prefix = '' OR string::starts_with(relative_path, $prefix));
"#,
            )
            .bind(("cutoff", cutoff))
            .bind(("prefix", prefix))
            .await
            .map_err(|error| format!("Could not read Location activity: {error}"))?
            .check()
            .map_err(|error| format!("Could not read Location activity: {error}"))?;
        let rows: Vec<ActivityDailyRow> = response
            .take(0)
            .map_err(|error| format!("Could not decode Location activity: {error}"))?;
        let mut grouped: HashMap<String, DocumentActivity> = HashMap::new();
        for row in rows {
            let activity = grouped
                .entry(row.relative_path.clone())
                .or_insert(DocumentActivity {
                    relative_path: row.relative_path,
                    changed_count: 0,
                    served_count: 0,
                    context_count: 0,
                    created_count: 0,
                    removed_count: 0,
                    last_changed_at: None,
                    last_served_at: None,
                });
            activity.changed_count += row.changed_count;
            activity.served_count += row.served_count;
            activity.context_count += row.context_count;
            activity.created_count += row.created_count;
            activity.removed_count += row.removed_count;
            if row.last_changed_at > activity.last_changed_at {
                activity.last_changed_at = row.last_changed_at;
            }
            if row.last_served_at > activity.last_served_at {
                activity.last_served_at = row.last_served_at;
            }
        }
        let mut documents = grouped.into_values().collect::<Vec<_>>();
        documents.sort_by(|left, right| {
            let left_total = left.changed_count
                + left.served_count
                + left.context_count
                + left.created_count
                + left.removed_count;
            let right_total = right.changed_count
                + right.served_count
                + right.context_count
                + right.created_count
                + right.removed_count;
            right_total
                .cmp(&left_total)
                .then_with(|| left.relative_path.cmp(&right.relative_path))
        });
        documents.truncate(limit);
        Ok(LocationActivityResponse {
            location_id: request.location_id,
            window_days: days,
            generated_at: Utc::now().to_rfc3339(),
            documents,
        })
    }

    pub(crate) async fn location_overview(
        &self,
        location_id: &str,
    ) -> Result<LocationOverview, String> {
        validate_location_id(location_id)?;
        let status = self.status(location_id).await?;
        let facets = self
            .search_facets(SearchFacetsRequest {
                location_ids: vec![location_id.to_string()],
            })
            .await?;
        let index = self.open(location_id).await?;
        let generation = status
            .active_generation
            .ok_or_else(|| "This Location has no complete index generation yet.".to_string())?;
        let rows = self.facet_rows(location_id).await?;
        let findings = rows.iter().map(|row| row.finding_count).sum();
        let mut link_response = index
            .db
            .query("SELECT status FROM document_link WHERE generation = $generation;")
            .bind(("generation", generation))
            .await
            .map_err(|error| format!("Could not read Location links: {error}"))?
            .check()
            .map_err(|error| format!("Could not read Location links: {error}"))?;
        let links: Vec<LinkStatusRow> = link_response
            .take(0)
            .map_err(|error| format!("Could not decode Location links: {error}"))?;
        let resolved_links = links.iter().filter(|row| row.status == "resolved").count();
        let unresolved_links = links
            .iter()
            .filter(|row| row.status == "unresolved")
            .count();
        let mut log_response = index
            .db
            .query(
                "SELECT relative_path, body FROM document WHERE generation = $generation AND kind = 'log';",
            )
            .bind(("generation", generation))
            .await
            .map_err(|error| format!("Could not read OKF logs: {error}"))?
            .check()
            .map_err(|error| format!("Could not read OKF logs: {error}"))?;
        let log_documents: Vec<LogDocumentRow> = log_response
            .take(0)
            .map_err(|error| format!("Could not decode OKF logs: {error}"))?;
        let mut recent_logs = log_documents
            .into_iter()
            .flat_map(|document| parse_log_entries(&document))
            .collect::<Vec<_>>();
        recent_logs.sort_by(|left, right| {
            right
                .date
                .cmp(&left.date)
                .then_with(|| left.relative_path.cmp(&right.relative_path))
        });
        recent_logs.truncate(20);
        let activity = self
            .location_activity(LocationActivityRequest {
                location_id: location_id.to_string(),
                days: 15,
                limit: 20,
                path_prefix: String::new(),
            })
            .await?;
        Ok(LocationOverview {
            location_id: location_id.to_string(),
            status,
            types: facets.types,
            tags: facets.tags,
            roles: facets.roles,
            statuses: facets.statuses,
            trust: facets.trust,
            resolved_links,
            unresolved_links,
            findings,
            recent_logs,
            activity,
        })
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

async fn lexical_fallback_search(
    db: &Surreal<Db>,
    location_id: &str,
    generation: i64,
    query: &str,
    filters: &KnowledgeSearchFilters,
    today: &str,
    limit: usize,
) -> Result<Vec<KnowledgeSearchResult>, String> {
    let mut offset = 0_i64;
    let batch_size = LEXICAL_FALLBACK_BATCH_SIZE as i64;
    let mut results = Vec::new();

    loop {
        let mut response = db
            .query(
                r#"
SELECT relative_path, title, description, type, tags, kind, status, trust_tier,
       stale_after, finding_count, body, headings_text, metadata_text, generation,
       0.0 AS title_score,
       0.0 AS description_score,
       0.0 AS type_score,
       0.0 AS tags_score,
       0.0 AS headings_score,
       0.0 AS path_score,
       0.0 AS body_score,
       0.0 AS metadata_score
FROM document
WHERE generation = $generation
  AND (array::len($types) = 0 OR type IN $types)
  AND (array::len($tags) = 0 OR array::len(array::intersect(tags, $tags)) > 0)
  AND (array::len($roles) = 0 OR kind IN $roles)
  AND (array::len($statuses) = 0 OR status IN $statuses)
  AND (array::len($trust) = 0 OR trust_tier IN $trust)
  AND ($path_prefix = '' OR string::starts_with(relative_path, $path_prefix))
  AND (
    $findings = 'any' OR
    ($findings = 'with' AND finding_count > 0) OR
    ($findings = 'without' AND finding_count = 0)
  )
  AND (
    array::len($freshness) = 0 OR
    ('stale' IN $freshness AND stale_after != none AND stale_after <= $today) OR
    ('current' IN $freshness AND stale_after != none AND stale_after > $today) OR
    ('unspecified' IN $freshness AND stale_after = none)
  )
ORDER BY relative_path ASC
LIMIT $batch_size START $offset;
"#,
            )
            .bind(("generation", generation))
            .bind(("types", filters.types.clone()))
            .bind(("tags", filters.tags.clone()))
            .bind(("roles", filters.roles.clone()))
            .bind(("statuses", filters.statuses.clone()))
            .bind(("trust", filters.trust.clone()))
            .bind(("freshness", filters.freshness.clone()))
            .bind(("path_prefix", normalize_relative_path(&filters.path_prefix)))
            .bind(("findings", filters.findings.clone()))
            .bind(("today", today.to_string()))
            .bind(("batch_size", batch_size))
            .bind(("offset", offset))
            .await
            .map_err(|_| "Could not run the local lexical search.".to_string())?
            .check()
            .map_err(|_| "Could not run the local lexical search.".to_string())?;
        let rows: Vec<LocalSearchRow> = response
            .take(0)
            .map_err(|_| "Could not read the local lexical results.".to_string())?;
        let row_count = rows.len();
        results.extend(
            rows.into_iter()
                .filter(|row| row_matches_query(row, query))
                .map(|row| knowledge_result(location_id, query, row)),
        );
        results.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.relative_path.cmp(&right.relative_path))
        });
        results.truncate(limit);

        if row_count < LEXICAL_FALLBACK_BATCH_SIZE {
            break;
        }
        offset += batch_size;
    }

    Ok(results)
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

fn validate_knowledge_search(request: &KnowledgeSearchRequest) -> Result<(), String> {
    if request.location_ids.is_empty() {
        return Err("Choose at least one Location to search.".to_string());
    }
    if request.location_ids.len() > 100 {
        return Err("Too many Locations were selected.".to_string());
    }
    for location_id in &request.location_ids {
        validate_location_id(location_id)?;
    }
    if request.query.len() > 1_000 {
        return Err("The search query is too long.".to_string());
    }
    if !matches!(
        request.filters.findings.as_str(),
        "any" | "with" | "without"
    ) {
        return Err("The findings filter is invalid.".to_string());
    }
    Ok(())
}

fn increment_count(counts: &mut HashMap<String, usize>, value: String) {
    if !value.is_empty() {
        *counts.entry(value).or_default() += 1;
    }
}

fn sorted_counts(counts: HashMap<String, usize>) -> Vec<FacetCount> {
    let mut values = counts
        .into_iter()
        .map(|(value, count)| FacetCount { value, count })
        .collect::<Vec<_>>();
    values.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.value.to_lowercase().cmp(&right.value.to_lowercase()))
    });
    values
}

fn related_direction_rank(direction: &str) -> u8 {
    match direction {
        "mutual" => 0,
        "outgoing" => 1,
        "incoming" => 2,
        _ => 3,
    }
}

fn context_pack_header(query: &str, max_characters: usize) -> String {
    let query_line = if query.is_empty() {
        "No query supplied.".to_string()
    } else {
        format!("Query: {query}")
    };
    format!("# Construct context pack\n\n{query_line}\n\nBudget: {max_characters} characters\n\n")
}

fn context_document_prefix(
    title: &str,
    location_id: &str,
    relative_path: &str,
    role: &str,
    reason: &str,
) -> String {
    format!(
        "## {title}\n\n- Location: `{location_id}`\n- Path: `{relative_path}`\n- Role: {role}\n- Included because: {reason}\n\n---\n\n"
    )
}

fn minimum_context_cost(documents: &[PreparedContextDocument]) -> usize {
    documents
        .iter()
        .map(|document| {
            let body_characters = document.body.chars().count();
            let excerpt_characters = body_characters.min(HARD_MIN_CONTEXT_EXCERPT_CHARACTERS);
            let suffix_characters = if excerpt_characters < body_characters {
                CONTEXT_TRUNCATION_NOTE.chars().count()
            } else {
                1
            };
            document.prefix.chars().count() + excerpt_characters + suffix_characters
        })
        .sum()
}

fn allocate_context_body_characters(lengths: &[usize], budget: usize) -> Vec<usize> {
    let mut allocations = vec![0; lengths.len()];
    let minimum_targets = lengths
        .iter()
        .map(|length| (*length).min(MIN_CONTEXT_EXCERPT_CHARACTERS))
        .collect::<Vec<_>>();
    let remaining = distribute_context_budget_evenly(&minimum_targets, &mut allocations, budget);
    distribute_context_budget_proportionally(lengths, &mut allocations, remaining);
    allocations
}

fn distribute_context_budget_evenly(
    targets: &[usize],
    allocations: &mut [usize],
    mut budget: usize,
) -> usize {
    loop {
        let active = targets
            .iter()
            .enumerate()
            .filter_map(|(index, target)| (allocations[index] < *target).then_some(index))
            .collect::<Vec<_>>();
        if active.is_empty() || budget == 0 {
            return budget;
        }
        let share = (budget / active.len()).max(1);
        for index in active {
            let addition = share
                .min(targets[index].saturating_sub(allocations[index]))
                .min(budget);
            allocations[index] += addition;
            budget -= addition;
            if budget == 0 {
                return 0;
            }
        }
    }
}

fn distribute_context_budget_proportionally(
    lengths: &[usize],
    allocations: &mut [usize],
    mut budget: usize,
) -> usize {
    let needs = lengths
        .iter()
        .enumerate()
        .map(|(index, length)| length.saturating_sub(allocations[index]))
        .collect::<Vec<_>>();
    let total_need = needs.iter().sum::<usize>();
    if total_need == 0 || budget == 0 {
        return budget;
    }
    if budget >= total_need {
        for (index, need) in needs.into_iter().enumerate() {
            allocations[index] += need;
        }
        return budget - total_need;
    }

    let original_budget = budget;
    let mut remainders = Vec::new();
    for (index, need) in needs.iter().copied().enumerate() {
        let weighted = original_budget * need;
        let addition = weighted / total_need;
        allocations[index] += addition;
        budget -= addition;
        remainders.push((weighted % total_need, need, index));
    }
    remainders.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| right.1.cmp(&left.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    for (_, _, index) in remainders {
        if budget == 0 {
            break;
        }
        if allocations[index] < lengths[index] {
            allocations[index] += 1;
            budget -= 1;
        }
    }
    budget
}

fn truncate_characters(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

fn freshness_for(stale_after: Option<&str>) -> String {
    let Some(stale_after) = stale_after else {
        return "unspecified".to_string();
    };
    let today = Utc::now().date_naive().format("%Y-%m-%d").to_string();
    if stale_after <= today.as_str() {
        "stale".to_string()
    } else {
        "current".to_string()
    }
}

fn can_skip_reconciliation(meta: &IndexMeta, minimum_interval_ms: u64) -> bool {
    if meta.indexer_version != INDEXER_VERSION
        || !meta.complete
        || meta.active_generation.is_none()
        || meta.state == IndexState::Indexing
    {
        return false;
    }
    let Some(last_reconciled_at) = meta.last_reconciled_at.as_deref() else {
        return false;
    };
    let Ok(last_reconciled_at) = chrono::DateTime::parse_from_rfc3339(last_reconciled_at) else {
        return false;
    };
    let elapsed_ms = Utc::now()
        .signed_duration_since(last_reconciled_at.with_timezone(&Utc))
        .num_milliseconds();
    elapsed_ms >= 0 && elapsed_ms < minimum_interval_ms.min(i64::MAX as u64) as i64
}

fn exact_phrase(query: &str) -> Option<&str> {
    let trimmed = query.trim();
    (trimmed.len() >= 2 && trimmed.starts_with('"') && trimmed.ends_with('"'))
        .then(|| &trimmed[1..trimmed.len() - 1])
        .filter(|value| !value.trim().is_empty())
}

fn fold_search_text(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .map(|character| match character {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'ç' => 'c',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ñ' => 'n',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ý' | 'ÿ' => 'y',
            value => value,
        })
        .collect()
}

fn row_contains_phrase(row: &LocalSearchRow, phrase: &str) -> bool {
    let phrase = fold_search_text(phrase);
    let tags = row.tags.join(" ");
    let found = [
        row.title.as_str(),
        row.description.as_deref().unwrap_or_default(),
        row.r#type.as_deref().unwrap_or_default(),
        tags.as_str(),
        row.headings_text.as_str(),
        row.relative_path.as_str(),
        row.body.as_str(),
        row.metadata_text.as_str(),
    ]
    .into_iter()
    .any(|value| fold_search_text(value).contains(&phrase));
    found
}

fn row_matches_query(row: &LocalSearchRow, query: &str) -> bool {
    if let Some(phrase) = exact_phrase(query) {
        return row_contains_phrase(row, phrase);
    }
    search_signals(row, query)
        .into_iter()
        .any(|(_, signal, _)| signal > 0.0)
}

fn field_matches(value: &str, query: &str) -> bool {
    let value = fold_search_text(value);
    if let Some(phrase) = exact_phrase(query) {
        return value.contains(&fold_search_text(phrase));
    }
    query_terms(query)
        .into_iter()
        .any(|term| value.contains(&term))
}

fn field_signal(score: f64, matched: bool) -> f64 {
    if matched {
        score.max(1.0)
    } else {
        score
    }
}

fn search_signals(row: &LocalSearchRow, query: &str) -> [(&'static str, f64, f64); 8] {
    [
        (
            "title",
            field_signal(row.title_score, field_matches(&row.title, query)),
            8.0,
        ),
        (
            "description",
            field_signal(
                row.description_score,
                row.description
                    .as_deref()
                    .map(|value| field_matches(value, query))
                    .unwrap_or(false),
            ),
            5.0,
        ),
        (
            "type",
            field_signal(
                row.type_score,
                row.r#type
                    .as_deref()
                    .map(|value| field_matches(value, query))
                    .unwrap_or(false),
            ),
            4.0,
        ),
        (
            "tags",
            field_signal(row.tags_score, field_matches(&row.tags.join(" "), query)),
            4.0,
        ),
        (
            "headings",
            field_signal(row.headings_score, field_matches(&row.headings_text, query)),
            3.0,
        ),
        (
            "path",
            field_signal(row.path_score, field_matches(&row.relative_path, query)),
            2.0,
        ),
        (
            "body",
            field_signal(row.body_score, field_matches(&row.body, query)),
            1.0,
        ),
        (
            "metadata",
            field_signal(row.metadata_score, field_matches(&row.metadata_text, query)),
            1.0,
        ),
    ]
}

fn weighted_score(row: &LocalSearchRow, query: &str) -> f64 {
    search_signals(row, query)
        .into_iter()
        .map(|(_, signal, weight)| signal * weight)
        .sum()
}

fn matched_fields(row: &LocalSearchRow, query: &str) -> Vec<String> {
    search_signals(row, query)
        .into_iter()
        .filter(|(_, signal, _)| *signal > 0.0)
        .map(|(field, _, _)| field.to_string())
        .collect()
}

fn match_reason(query: &str, row: &LocalSearchRow) -> String {
    if row.title.eq_ignore_ascii_case(query.trim_matches('"')) {
        return "Exact title match".to_string();
    }
    search_signals(row, query)
        .into_iter()
        .filter(|(_, signal, _)| *signal > 0.0)
        .max_by(|left, right| (left.1 * left.2).total_cmp(&(right.1 * right.2)))
        .map(|(field, _, _)| match field {
            "title" => "Title match",
            "description" => "Description match",
            "type" => "Type match",
            "tags" => "Tag match",
            "headings" => "Heading match",
            "path" => "Path match",
            "body" => "Body match",
            "metadata" => "Frontmatter match",
            _ => "Knowledge match",
        })
        .map(str::to_string)
        .unwrap_or_else(|| "Knowledge match".to_string())
}

fn query_terms(query: &str) -> Vec<String> {
    query
        .split_whitespace()
        .map(|term| {
            term.trim_matches(|character: char| {
                character == '"' || character.is_ascii_punctuation()
            })
        })
        .filter(|term| term.len() >= 2)
        .map(fold_search_text)
        .collect()
}

fn char_boundary_before(value: &str, mut index: usize) -> usize {
    index = index.min(value.len());
    while index > 0 && !value.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn char_boundary_after(value: &str, mut index: usize) -> usize {
    index = index.min(value.len());
    while index < value.len() && !value.is_char_boundary(index) {
        index += 1;
    }
    index
}

fn folded_search_text_with_offsets(value: &str) -> (String, Vec<usize>) {
    let mut folded = String::new();
    let mut offsets = Vec::new();
    for (offset, character) in value.char_indices() {
        let folded_character = fold_search_text(&character.to_string());
        offsets.extend(std::iter::repeat_n(offset, folded_character.len()));
        folded.push_str(&folded_character);
    }
    offsets.push(value.len());
    (folded, offsets)
}

fn search_snippet(body: &str, description: Option<&str>, query: &str) -> String {
    let source = if body.trim().is_empty() {
        description.unwrap_or_default()
    } else {
        body
    };
    let collapsed = source.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.is_empty() {
        return String::new();
    }
    let (folded, offsets) = folded_search_text_with_offsets(&collapsed);
    let folded_position = query_terms(query)
        .iter()
        .filter_map(|term| folded.find(term))
        .min()
        .unwrap_or_default();
    let position = offsets.get(folded_position).copied().unwrap_or_default();
    let start = char_boundary_before(&collapsed, position.saturating_sub(90));
    let end = char_boundary_after(&collapsed, (position + 190).min(collapsed.len()));
    let mut snippet = collapsed[start..end].trim().to_string();
    if start > 0 {
        snippet.insert(0, '…');
    }
    if end < collapsed.len() {
        snippet.push('…');
    }
    snippet
}

fn knowledge_result(location_id: &str, query: &str, row: LocalSearchRow) -> KnowledgeSearchResult {
    let snippet = search_snippet(&row.body, row.description.as_deref(), query);
    let matched_fields = matched_fields(&row, query);
    let match_reason = match_reason(query, &row);
    let score = weighted_score(&row, query);
    let freshness = freshness_for(row.stale_after.as_deref());
    KnowledgeSearchResult {
        location_id: location_id.to_string(),
        relative_path: row.relative_path,
        title: row.title,
        description: row.description,
        r#type: row.r#type,
        tags: row.tags,
        role: row.kind,
        status: row.status,
        trust: row.trust_tier,
        freshness,
        stale_after: row.stale_after,
        finding_count: row.finding_count,
        snippet,
        matched_fields,
        match_reason,
        score,
        rank_score: 0.0,
        generation: row.generation,
    }
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

fn flatten_okf_value(value: &Value, output: &mut Vec<String>) {
    match value.get("kind").and_then(Value::as_str) {
        Some("boolean" | "integer" | "unsignedInteger" | "float" | "string") => {
            if let Some(value) = value.get("value") {
                match value {
                    Value::String(value) => output.push(value.clone()),
                    Value::Bool(value) => output.push(value.to_string()),
                    Value::Number(value) => output.push(value.to_string()),
                    _ => {}
                }
            }
        }
        Some("sequence") => {
            if let Some(items) = value.get("items").and_then(Value::as_array) {
                for item in items {
                    flatten_okf_value(item, output);
                }
            }
        }
        Some("mapping") => {
            if let Some(entries) = value.get("entries").and_then(Value::as_array) {
                for entry in entries {
                    if let Some(key) = entry.get("key") {
                        flatten_okf_value(key, output);
                    }
                    if let Some(value) = entry.get("value") {
                        flatten_okf_value(value, output);
                    }
                }
            }
        }
        Some("tagged") => {
            if let Some(value) = value.get("value") {
                flatten_okf_value(value, output);
            }
        }
        _ => {}
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

async fn record_daily_activity(
    db: &Surreal<Db>,
    relative_path: &str,
    kind: &str,
    now: &str,
) -> Result<(), String> {
    let day = Utc::now().date_naive().format("%Y-%m-%d").to_string();
    let record_id = blake3::hash(format!("{day}\0{relative_path}").as_bytes())
        .to_hex()
        .to_string();
    let mut row: ActivityDailyRow = db
        .select(("document_activity_daily", record_id.as_str()))
        .await
        .map_err(|error| format!("Could not read document activity: {error}"))?
        .unwrap_or(ActivityDailyRow {
            relative_path: relative_path.to_string(),
            day,
            changed_count: 0,
            served_count: 0,
            context_count: 0,
            created_count: 0,
            removed_count: 0,
            last_changed_at: None,
            last_served_at: None,
        });
    match kind {
        "changed" => {
            row.changed_count += 1;
            row.last_changed_at = Some(now.to_string());
        }
        "created" => {
            row.created_count += 1;
            row.last_changed_at = Some(now.to_string());
        }
        "removed" => {
            row.removed_count += 1;
            row.last_changed_at = Some(now.to_string());
        }
        "served" => {
            row.served_count += 1;
            row.last_served_at = Some(now.to_string());
        }
        "context" => {
            row.context_count += 1;
            row.last_served_at = Some(now.to_string());
        }
        _ => return Err("The document activity kind is invalid.".to_string()),
    }
    let _: Option<ActivityDailyRow> = db
        .upsert(("document_activity_daily", record_id.as_str()))
        .content(row)
        .await
        .map_err(|error| format!("Could not update document activity: {error}"))?;
    Ok(())
}

async fn cleanup_activity(db: &Surreal<Db>) -> Result<(), String> {
    let cutoff = (Utc::now().date_naive() - chrono::Duration::days(14))
        .format("%Y-%m-%d")
        .to_string();
    db.query("DELETE document_activity_daily WHERE day < $cutoff;")
        .bind(("cutoff", cutoff))
        .await
        .map_err(|error| format!("Could not clean up document activity: {error}"))?
        .check()
        .map_err(|error| format!("Could not clean up document activity: {error}"))?;
    Ok(())
}

fn parse_log_entries(document: &LogDocumentRow) -> Vec<LogEntry> {
    let scope = document
        .relative_path
        .rsplit_once('/')
        .map(|(parent, _)| parent.to_string())
        .unwrap_or_else(|| "root".to_string());
    let mut entries = Vec::new();
    let mut date: Option<String> = None;
    let mut summary = Vec::new();
    let flush = |entries: &mut Vec<LogEntry>, date: &Option<String>, summary: &mut Vec<String>| {
        if summary.is_empty() {
            return;
        }
        let text = summary.join(" ");
        let text = truncate_characters(text.trim(), 500);
        if !text.is_empty() {
            entries.push(LogEntry {
                relative_path: document.relative_path.clone(),
                scope: scope.clone(),
                date: date.clone(),
                summary: text,
            });
        }
        summary.clear();
    };
    for line in document.body.lines() {
        let trimmed = line.trim();
        if let Some(heading) = trimmed.strip_prefix('#') {
            let heading = heading.trim_start_matches('#').trim();
            let candidate = heading
                .split_whitespace()
                .next()
                .unwrap_or_default()
                .trim_matches(|character: char| !character.is_ascii_digit() && character != '-');
            if candidate.len() >= 10
                && candidate.as_bytes().get(4) == Some(&b'-')
                && candidate.as_bytes().get(7) == Some(&b'-')
            {
                flush(&mut entries, &date, &mut summary);
                date = Some(candidate[..10].to_string());
                continue;
            }
        }
        if trimmed.is_empty() {
            if !summary.is_empty() {
                flush(&mut entries, &date, &mut summary);
            }
            continue;
        }
        if !trimmed.starts_with("---") {
            summary.push(
                trimmed
                    .trim_start_matches(['-', '*', '+'])
                    .trim()
                    .to_string(),
            );
        }
    }
    flush(&mut entries, &date, &mut summary);
    entries
}

fn okf_typed_string(value: &Value) -> Option<&str> {
    (value.get("kind").and_then(Value::as_str) == Some("string"))
        .then(|| value.get("value").and_then(Value::as_str))
        .flatten()
}

fn collect_okf_mapping_field(value: &Value, field: &str, output: &mut Vec<String>) {
    match value.get("kind").and_then(Value::as_str) {
        Some("mapping") => {
            if let Some(entries) = value.get("entries").and_then(Value::as_array) {
                for entry in entries {
                    let key = entry.get("key").and_then(okf_typed_string);
                    let entry_value = entry.get("value");
                    if key == Some(field) {
                        if let Some(value) = entry_value.and_then(okf_typed_string) {
                            output.push(value.to_string());
                        }
                    }
                    if let Some(value) = entry_value {
                        collect_okf_mapping_field(value, field, output);
                    }
                }
            }
        }
        Some("sequence") => {
            if let Some(items) = value.get("items").and_then(Value::as_array) {
                for item in items {
                    collect_okf_mapping_field(item, field, output);
                }
            }
        }
        Some("tagged") => {
            if let Some(value) = value.get("value") {
                collect_okf_mapping_field(value, field, output);
            }
        }
        _ => {}
    }
}

fn trust_tier(metadata: &Value, is_okf_concept: bool) -> Option<String> {
    if !is_okf_concept {
        return None;
    }
    let mut actors = Vec::new();
    if let Some(verified) = metadata.get("verified") {
        collect_okf_mapping_field(verified, "by", &mut actors);
    }
    if actors.iter().any(|actor| actor.starts_with("human:")) {
        Some("humanReviewed".to_string())
    } else if actors.is_empty() {
        Some("unverified".to_string())
    } else {
        Some("machineConfirmed".to_string())
    }
}

struct DocumentBuildContext<'a> {
    request: &'a SyncLocationRequest,
    root: &'a Path,
    discovered_paths: &'a HashSet<String>,
    generation: i64,
}

fn build_document(
    context: &DocumentBuildContext<'_>,
    entry: &MarkdownEntry,
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
        context.root,
        entry.relative_path.eq_ignore_ascii_case("index.md"),
    );
    let links = okf::indexable_links(&inspection, context.root)
        .into_iter()
        .map(|link| {
            let mut status = link.status;
            if status == "candidate" {
                status = match link.target_relative_path.as_ref() {
                    Some(path) if context.discovered_paths.contains(path) => "resolved",
                    Some(_) => "unresolved",
                    None => "candidate",
                }
                .to_string();
            }
            IndexedLink {
                location_id: context.request.location_id.clone(),
                generation: context.generation,
                source_relative_path: entry.relative_path.clone(),
                target: link.target,
                target_relative_path: link.target_relative_path,
                fragment: link.fragment,
                origin: link.origin,
                field: link.field,
                status,
                start_line: link.start_line,
                end_line: link.end_line,
            }
        })
        .collect::<Vec<_>>();
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
    let is_okf_concept = context.request.okf_bundle && kind == "concept";
    let lifecycle_status = if is_okf_concept {
        Some(json_string(&metadata, "status").unwrap_or_else(|| "stable".to_string()))
    } else {
        None
    };
    let stale_after = is_okf_concept
        .then(|| json_string(&metadata, "staleAfter"))
        .flatten();
    let trust_tier = trust_tier(&metadata, is_okf_concept);
    let finding_count = inspection_json
        .get("findings")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let headings_text = headings
        .iter()
        .map(|heading| heading.text.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let mut metadata_projection = Vec::new();
    if let Some(value) = frontmatter.as_ref() {
        flatten_okf_value(value, &mut metadata_projection);
    }
    let metadata_text = metadata_projection.join(" ");
    let description_text = description.clone().unwrap_or_default();
    let type_text = document_type.clone().unwrap_or_default();
    let tags_text = tags.join(" ");
    let relative_path_search = entry.relative_path.clone();
    let mut projection = vec![
        title.clone(),
        description_text.clone(),
        type_text.clone(),
        tags_text.clone(),
        relative_path_search.clone(),
        headings_text.clone(),
        body.clone(),
        metadata_text.clone(),
    ];
    projection.retain(|value| !value.is_empty());
    IndexedDocument {
        location_id: context.request.location_id.clone(),
        generation: context.generation,
        relative_path: entry.relative_path.clone(),
        continuity_id,
        content_hash,
        modified_at_ms: entry.modified_at_ms,
        size: entry.size,
        kind,
        title,
        description,
        description_text,
        r#type: document_type,
        type_text,
        tags,
        tags_text,
        headings,
        headings_text,
        frontmatter,
        body,
        relative_path_search,
        metadata_text,
        search_text: projection.join("\n"),
        okf: context.request.okf_bundle.then_some(inspection_json),
        parse_error: None,
        status: lifecycle_status,
        trust_tier,
        stale_after,
        finding_count,
        links,
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
        "DELETE document_link WHERE generation = $generation AND source_relative_path IN $changed_or_removed;\n",
    );
    let mut link_position = 0usize;
    for document in documents {
        for _ in &document.links {
            query.push_str(&format!(
                "UPSERT type::record('document_link', $link_record_id_{link_position}) CONTENT $link_{link_position};\n"
            ));
            link_position += 1;
        }
    }
    query.push_str(
        "DELETE document WHERE generation = $generation AND relative_path IN $removed;\nCOMMIT TRANSACTION;",
    );
    let changed_or_removed = documents
        .iter()
        .map(|document| document.relative_path.clone())
        .chain(removed.iter().cloned())
        .collect::<Vec<_>>();
    let mut pending = db
        .query(query)
        .bind(("generation", generation))
        .bind(("removed", removed.to_vec()))
        .bind(("changed_or_removed", changed_or_removed));
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
    let mut link_position = 0usize;
    for document in documents {
        for link in &document.links {
            let mut hasher = Hasher::new();
            hasher.update(link.generation.to_string().as_bytes());
            hasher.update(b":");
            hasher.update(link.source_relative_path.as_bytes());
            hasher.update(b":");
            hasher.update(link_position.to_string().as_bytes());
            hasher.update(b":");
            hasher.update(link.target.as_bytes());
            let record_id = hasher.finalize().to_hex().to_string();
            pending = pending
                .bind((format!("link_record_id_{link_position}"), record_id))
                .bind((format!("link_{link_position}"), link.clone()));
            link_position += 1;
        }
    }
    pending
        .await
        .map_err(|error| format!("Could not update the Location index: {error}"))?
        .check()
        .map_err(|error| format!("Could not update the Location index: {error}"))?;
    Ok(())
}

async fn refresh_link_resolutions(
    db: &Surreal<Db>,
    generation: i64,
    discovered_paths: &HashSet<String>,
) -> Result<(), String> {
    let paths = discovered_paths.iter().cloned().collect::<Vec<_>>();
    db.query(
        r#"
BEGIN TRANSACTION;
UPDATE document_link
SET status = 'unresolved'
WHERE generation = $generation
  AND target_relative_path != NONE
  AND status != 'fragment';
UPDATE document_link
SET status = 'resolved'
WHERE generation = $generation
  AND target_relative_path IN $paths
  AND status != 'fragment';
COMMIT TRANSACTION;
"#,
    )
    .bind(("generation", generation))
    .bind(("paths", paths))
    .await
    .map_err(|error| format!("Could not refresh Markdown link resolution: {error}"))?
    .check()
    .map_err(|error| format!("Could not refresh Markdown link resolution: {error}"))?;
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
    db.query(
        r#"
DELETE document WHERE generation != $active_generation;
DELETE document_link WHERE generation != $active_generation;
"#,
    )
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
            minimum_reconcile_interval_ms: 0,
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
    async fn searches_and_filters_federated_okf_metadata() {
        let data = temporary_root("search-data");
        let first = temporary_root("search-first");
        let second = temporary_root("search-second");
        fs::write(
            first.join("luis.md"),
            "---\ntype: Person\ntitle: Luis Novo\ndescription: SurrealDB ambassador\ntags: [team, strategy]\nstatus: draft\nstale_after: 2020-01-01\nverified: { by: human:luis, at: 2026-07-26T00:00:00Z }\n---\n# Luis\nBuilds the órbita and orbital knowledge system.",
        )
        .expect("write person");
        fs::write(
            first.join("construct.md"),
            "---\ntype: Project\ntitle: Construct\ntags: [team, software]\nstatus: stable\nverified: { by: process:nightly, at: 2026-07-26T00:00:00Z }\n---\n# Construct\nAn orbital Markdown workspace.",
        )
        .expect("write project");
        fs::write(
            second.join("knowledge.md"),
            "---\ntype: Person\ntitle: Knowledge Keeper\ntags: [strategy]\n---\n# Knowledge Keeper\nMaintains another orbital corpus.",
        )
        .expect("write second person");
        let service = IndexService::new(data.join("indexes")).expect("create service");
        let mut first_request = request("search-first", &first);
        first_request.okf_bundle = true;
        let mut second_request = request("search-second", &second);
        second_request.okf_bundle = true;
        service
            .sync(first_request, first.clone())
            .await
            .expect("index first");
        service
            .sync(second_request, second.clone())
            .await
            .expect("index second");

        let response = service
            .search_knowledge(KnowledgeSearchRequest {
                location_ids: vec!["search-first".to_string(), "search-second".to_string()],
                query: "orbital".to_string(),
                filters: KnowledgeSearchFilters {
                    types: vec!["Person".to_string()],
                    tags: vec!["strategy".to_string()],
                    ..KnowledgeSearchFilters::default()
                },
                limit: 20,
            })
            .await
            .expect("search knowledge");
        assert_eq!(response.results.len(), 2);
        assert!(response.unavailable_location_ids.is_empty());
        assert_eq!(response.results[0].location_id, "search-first");
        assert!(response
            .results
            .iter()
            .all(|result| result.r#type.as_deref() == Some("Person")));
        let luis = response
            .results
            .iter()
            .find(|result| result.title == "Luis Novo")
            .expect("find Luis");
        assert_eq!(luis.status.as_deref(), Some("draft"));
        assert_eq!(luis.trust.as_deref(), Some("humanReviewed"));
        assert_eq!(luis.freshness, "stale");
        assert!(luis.matched_fields.contains(&"body".to_string()));
        assert_eq!(luis.match_reason, "Body match");
        assert!(luis.snippet.contains("orbital"));

        let lifecycle = service
            .search_knowledge(KnowledgeSearchRequest {
                location_ids: vec!["search-first".to_string()],
                query: "\"orbital knowledge\"".to_string(),
                filters: KnowledgeSearchFilters {
                    statuses: vec!["draft".to_string()],
                    trust: vec!["humanReviewed".to_string()],
                    freshness: vec!["stale".to_string()],
                    findings: "without".to_string(),
                    ..KnowledgeSearchFilters::default()
                },
                limit: 20,
            })
            .await
            .expect("filter lifecycle metadata");
        assert_eq!(lifecycle.results.len(), 1);
        assert_eq!(lifecycle.results[0].title, "Luis Novo");

        let accent_insensitive = service
            .search_knowledge(KnowledgeSearchRequest {
                location_ids: vec!["search-first".to_string()],
                query: "orbita".to_string(),
                filters: KnowledgeSearchFilters::default(),
                limit: 20,
            })
            .await
            .expect("search without accent");
        assert_eq!(accent_insensitive.results.len(), 1);
        assert_eq!(accent_insensitive.results[0].title, "Luis Novo");
        assert!(accent_insensitive.results[0]
            .matched_fields
            .contains(&"body".to_string()));

        let facets = service
            .search_facets(SearchFacetsRequest {
                location_ids: vec!["search-first".to_string(), "search-second".to_string()],
            })
            .await
            .expect("read facets");
        assert_eq!(
            facets
                .types
                .iter()
                .find(|facet| facet.value == "Person")
                .map(|facet| facet.count),
            Some(2)
        );
        assert_eq!(
            facets
                .trust
                .iter()
                .find(|facet| facet.value == "unverified")
                .map(|facet| facet.count),
            Some(1)
        );

        drop(service);
        fs::remove_dir_all(data).expect("remove data");
        fs::remove_dir_all(first).expect("remove first");
        fs::remove_dir_all(second).expect("remove second");
    }

    #[tokio::test]
    async fn falls_back_to_local_lexical_search_without_fulltext_indexes() {
        let data = temporary_root("fallback-search-data");
        let source = temporary_root("fallback-search-source");
        fs::write(
            source.join("construct.md"),
            "---\ntype: Project\ntitle: Construct Search\ntags: [windows, local]\n---\n# Search\nA recuperação encontra conteúdo mesmo sem o índice full-text.",
        )
        .expect("write searchable document");
        let location_id = "fallback-search-location";
        let service = IndexService::new(data.join("indexes")).expect("create service");
        service
            .sync(request(location_id, &source), source.clone())
            .await
            .expect("index document");

        let index = service.open(location_id).await.expect("open index");
        index
            .db
            .query(
                r#"
REMOVE INDEX IF EXISTS document_search ON TABLE document;
REMOVE INDEX IF EXISTS document_title_search ON TABLE document;
REMOVE INDEX IF EXISTS document_description_search ON TABLE document;
REMOVE INDEX IF EXISTS document_type_search ON TABLE document;
REMOVE INDEX IF EXISTS document_tags_search ON TABLE document;
REMOVE INDEX IF EXISTS document_headings_search ON TABLE document;
REMOVE INDEX IF EXISTS document_path_search ON TABLE document;
REMOVE INDEX IF EXISTS document_body_search ON TABLE document;
REMOVE INDEX IF EXISTS document_metadata_search ON TABLE document;
"#,
            )
            .await
            .expect("remove fulltext indexes")
            .check()
            .expect("apply index removal");

        let response = service
            .search_knowledge(KnowledgeSearchRequest {
                location_ids: vec![location_id.to_string()],
                query: "recuperacao".to_string(),
                filters: KnowledgeSearchFilters {
                    types: vec!["Project".to_string()],
                    tags: vec!["windows".to_string()],
                    ..KnowledgeSearchFilters::default()
                },
                limit: 20,
            })
            .await
            .expect("search through lexical fallback");
        assert!(response.unavailable_location_ids.is_empty());
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].relative_path, "construct.md");
        assert_eq!(response.results[0].match_reason, "Body match");

        let legacy = service
            .search(SearchIndexRequest {
                location_id: location_id.to_string(),
                query: "recuperacao".to_string(),
                limit: 20,
            })
            .await
            .expect("search legacy endpoint through lexical fallback");
        assert_eq!(legacy.len(), 1);
        assert_eq!(legacy[0].relative_path, "construct.md");

        drop(index);
        drop(service);
        fs::remove_dir_all(data).expect("remove data");
        fs::remove_dir_all(source).expect("remove source");
    }

    #[test]
    fn creates_bounded_unicode_snippets() {
        let body = "Introdução ".repeat(40) + "órbita relevante para o contexto";
        let snippet = search_snippet(&body, None, "órbita");
        assert!(snippet.starts_with('…'));
        assert!(snippet.contains("órbita relevante"));
        assert!(snippet.len() < body.len());
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
        let unchanged = service
            .sync(request(location_id, &source), source.clone())
            .await
            .expect("no-op sync");
        assert_eq!(unchanged.state, IndexState::Ready);
        assert_eq!(unchanged.changed_documents, 0);
        assert_eq!(unchanged.removed_documents, 0);
        assert_eq!(unchanged.indexed_documents, 2);

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

    #[tokio::test]
    async fn coalesces_recent_background_reconciliations() {
        let data = temporary_root("coalesce-data");
        let source = temporary_root("coalesce-source");
        fs::write(source.join("one.md"), "# One\nBefore").expect("write one");
        let location_id = "coalesce-location";
        let service = IndexService::new(data.join("indexes")).expect("create service");
        service
            .sync(request(location_id, &source), source.clone())
            .await
            .expect("initial sync");

        fs::write(source.join("one.md"), "# One\nAfter").expect("change one");
        let mut background_request = request(location_id, &source);
        background_request.minimum_reconcile_interval_ms = 60_000;
        let coalesced = service
            .sync(background_request, source.clone())
            .await
            .expect("coalesce recent sync");
        assert_eq!(coalesced.state, IndexState::Ready);
        assert!(coalesced.complete);
        assert_eq!(coalesced.changed_documents, 0);
        let unchanged = service
            .get_document(location_id, "one.md")
            .await
            .expect("read coalesced document")
            .expect("document exists");
        assert!(unchanged.body.contains("Before"));

        let refreshed = service
            .sync(request(location_id, &source), source.clone())
            .await
            .expect("force next sync");
        assert_eq!(refreshed.state, IndexState::Ready);
        assert_eq!(refreshed.changed_documents, 1);
        let changed = service
            .get_document(location_id, "one.md")
            .await
            .expect("read refreshed document")
            .expect("document exists");
        assert!(changed.body.contains("After"));

        drop(service);
        fs::remove_dir_all(data).expect("remove data");
        fs::remove_dir_all(source).expect("remove source");
    }

    #[tokio::test]
    async fn persists_direct_links_and_explains_both_directions() {
        let data = temporary_root("links-data");
        let source = temporary_root("links-source");
        fs::write(
            source.join("alpha.md"),
            "# Alpha\n\nSee [Beta](beta.md).\n\n<!-- construct-review:v1\n{\"comments\":[{\"id\":\"1\",\"quote\":\"Alpha\",\"comment\":\"See [Hidden](hidden.md)\",\"createdAt\":\"2026-07-26T00:00:00Z\"}]}\n-->",
        )
        .expect("write alpha");
        fs::write(source.join("beta.md"), "# Beta\n\nSupporting knowledge.").expect("write beta");
        fs::write(source.join("hidden.md"), "# Hidden\n\nReview-only target.")
            .expect("write hidden");
        let service = IndexService::new(data.join("indexes")).expect("create service");
        service
            .sync(request("links-location", &source), source.clone())
            .await
            .expect("index links");

        let from_alpha = service
            .related_documents(RelatedDocumentsRequest {
                location_id: "links-location".to_string(),
                relative_path: "alpha.md".to_string(),
                limit: 20,
            })
            .await
            .expect("read outgoing links");
        assert_eq!(from_alpha.documents.len(), 1);
        assert_eq!(from_alpha.documents[0].relative_path, "beta.md");
        assert_eq!(from_alpha.documents[0].direction, "outgoing");
        assert_eq!(from_alpha.documents[0].reason, "Linked from alpha.md");

        let from_beta = service
            .related_documents(RelatedDocumentsRequest {
                location_id: "links-location".to_string(),
                relative_path: "beta.md".to_string(),
                limit: 20,
            })
            .await
            .expect("read backlinks");
        assert_eq!(from_beta.documents.len(), 1);
        assert_eq!(from_beta.documents[0].relative_path, "alpha.md");
        assert_eq!(from_beta.documents[0].direction, "incoming");
        assert_eq!(from_beta.documents[0].reason, "Links to beta.md");

        drop(service);
        fs::remove_dir_all(data).expect("remove data");
        fs::remove_dir_all(source).expect("remove source");
    }

    #[tokio::test]
    async fn assembles_context_with_provenance_and_a_character_budget() {
        let data = temporary_root("context-data");
        let source = temporary_root("context-source");
        fs::write(
            source.join("first.md"),
            format!("# First\n\n{}", "Órbita de contexto. ".repeat(200)),
        )
        .expect("write first");
        fs::write(source.join("second.md"), "# Second\n\nAdditional context.")
            .expect("write second");
        let service = IndexService::new(data.join("indexes")).expect("create service");
        service
            .sync(request("context-location", &source), source.clone())
            .await
            .expect("index context");

        let pack = service
            .build_context_pack(BuildContextPackRequest {
                query: "orbital context".to_string(),
                documents: vec![
                    ContextDocumentRef {
                        location_id: "context-location".to_string(),
                        relative_path: "first.md".to_string(),
                        reason: "Body match".to_string(),
                    },
                    ContextDocumentRef {
                        location_id: "context-location".to_string(),
                        relative_path: "second.md".to_string(),
                        reason: "Linked from first.md".to_string(),
                    },
                ],
                max_characters: 1_000,
                max_documents: 1,
            })
            .await
            .expect("build context pack");

        assert_eq!(pack.items.len(), 1);
        assert!(pack.items[0].truncated);
        assert_eq!(pack.omitted.len(), 1);
        assert!(pack.total_characters <= 1_000);
        assert_eq!(pack.total_characters, pack.markdown.chars().count());
        assert!(pack.markdown.contains("Location: `context-location`"));
        assert!(pack.markdown.contains("Path: `first.md`"));
        assert!(pack.markdown.contains("Content truncated"));
        assert!(!pack
            .markdown
            .contains(&source.to_string_lossy().to_string()));

        drop(service);
        fs::remove_dir_all(data).expect("remove data");
        fs::remove_dir_all(source).expect("remove source");
    }

    #[tokio::test]
    async fn balances_context_across_documents_before_growing_large_excerpts() {
        let data = temporary_root("balanced-context-data");
        let source = temporary_root("balanced-context-source");
        fs::write(source.join("small.md"), "# Small\n\nBrief context.").expect("write small");
        fs::write(
            source.join("medium.md"),
            format!("# Medium\n\n{}", "Medium context. ".repeat(120)),
        )
        .expect("write medium");
        fs::write(
            source.join("large.md"),
            format!(
                "# Large\n\n{}",
                "Large context with more detail. ".repeat(220)
            ),
        )
        .expect("write large");
        let service = IndexService::new(data.join("indexes")).expect("create service");
        service
            .sync(
                request("balanced-context-location", &source),
                source.clone(),
            )
            .await
            .expect("index context");

        let pack = service
            .build_context_pack(BuildContextPackRequest {
                query: "balanced context".to_string(),
                documents: vec![
                    ContextDocumentRef {
                        location_id: "balanced-context-location".to_string(),
                        relative_path: "small.md".to_string(),
                        reason: "Small source".to_string(),
                    },
                    ContextDocumentRef {
                        location_id: "balanced-context-location".to_string(),
                        relative_path: "medium.md".to_string(),
                        reason: "Medium source".to_string(),
                    },
                    ContextDocumentRef {
                        location_id: "balanced-context-location".to_string(),
                        relative_path: "large.md".to_string(),
                        reason: "Large source".to_string(),
                    },
                ],
                max_characters: 1_600,
                max_documents: 3,
            })
            .await
            .expect("build balanced context pack");

        assert_eq!(pack.items.len(), 3);
        assert!(pack.omitted.is_empty());
        assert!(!pack.items[0].truncated);
        assert!(pack.items[1].truncated);
        assert!(pack.items[2].truncated);
        assert!(pack.items[1].content.chars().count() >= MIN_CONTEXT_EXCERPT_CHARACTERS);
        assert!(pack.items[2].content.chars().count() > pack.items[1].content.chars().count());
        assert!(pack.total_characters <= 1_600);
        assert!(pack.markdown.contains("## Small"));
        assert!(pack.markdown.contains("## Medium"));
        assert!(pack.markdown.contains("## Large"));

        drop(service);
        fs::remove_dir_all(data).expect("remove data");
        fs::remove_dir_all(source).expect("remove source");
    }

    #[tokio::test]
    async fn keeps_hot_activity_separate_and_reads_nested_okf_logs() {
        let data = temporary_root("activity-data");
        let source = temporary_root("activity-source");
        fs::create_dir_all(source.join("projects")).expect("create nested scope");
        fs::write(source.join("index.md"), "okf_version: 0.2\n").expect("write index");
        fs::write(source.join("concept.md"), "# Concept\n\nInitial body.").expect("write concept");
        fs::write(
            source.join("projects/log.md"),
            "# Project log\n\n## 2026-07-26\n\n- Added the retrieval service.\n\n## 2026-07-25\n\n- Prepared the corpus.",
        )
        .expect("write log");
        let service = IndexService::new(data.join("indexes")).expect("create service");
        let mut sync_request = request("activity-location", &source);
        sync_request.okf_bundle = true;
        service
            .sync(sync_request.clone(), source.clone())
            .await
            .expect("initial index");
        let empty = service
            .location_activity(LocationActivityRequest {
                location_id: "activity-location".to_string(),
                days: 15,
                limit: 20,
                path_prefix: String::new(),
            })
            .await
            .expect("read empty activity");
        assert!(
            empty.documents.is_empty(),
            "rebuilds must not heat activity"
        );

        fs::write(source.join("concept.md"), "# Concept\n\nChanged body.").expect("change concept");
        service
            .sync(sync_request, source.clone())
            .await
            .expect("incremental index");
        service
            .record_document_activity("activity-location", "concept.md", ActivityKind::Served)
            .await
            .expect("record served");
        service
            .record_document_activity("activity-location", "concept.md", ActivityKind::Context)
            .await
            .expect("record context");

        let overview = service
            .location_overview("activity-location")
            .await
            .expect("read overview");
        let activity = overview
            .activity
            .documents
            .iter()
            .find(|document| document.relative_path == "concept.md")
            .expect("find activity");
        assert_eq!(activity.changed_count, 1);
        assert_eq!(activity.served_count, 1);
        assert_eq!(activity.context_count, 1);
        assert_eq!(activity.created_count, 0);
        assert_eq!(overview.recent_logs[0].date.as_deref(), Some("2026-07-26"));
        assert_eq!(overview.recent_logs[0].scope, "projects");
        assert!(overview.recent_logs[0]
            .summary
            .contains("retrieval service"));

        drop(service);
        fs::remove_dir_all(data).expect("remove data");
        fs::remove_dir_all(source).expect("remove source");
    }
}
