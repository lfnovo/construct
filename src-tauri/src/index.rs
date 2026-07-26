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

const INDEXER_VERSION: i64 = 2;
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

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct KnowledgeSearchRequest {
    pub(crate) location_ids: Vec<String>,
    pub(crate) query: String,
    #[serde(default)]
    pub(crate) filters: KnowledgeSearchFilters,
    #[serde(default = "default_search_limit")]
    pub(crate) limit: usize,
}

#[derive(Clone, Debug, Deserialize)]
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
    pub(crate) headings: Vec<Heading>,
    pub(crate) frontmatter: Option<Value>,
    pub(crate) body: String,
    pub(crate) generation: i64,
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
        let mut response = index
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
            .bind(("today", today))
            .bind(("limit", limit as i64))
            .await
            .map_err(|_| "Could not search this Location. Try a simpler query.".to_string())?
            .check()
            .map_err(|_| "Could not search this Location. Try a simpler query.".to_string())?;
        let rows: Vec<LocalSearchRow> = response
            .take(0)
            .map_err(|_| "Could not read the local search results.".to_string())?;
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
                "SELECT type, tags, kind, status, trust_tier, stale_after FROM document WHERE generation = $generation;",
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
    let is_okf_concept = request.okf_bundle && kind == "concept";
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
        okf: request.okf_bundle.then_some(inspection_json),
        parse_error: None,
        status: lifecycle_status,
        trust_tier,
        stale_after,
        finding_count,
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
