use crate::{
    index::{
        BuildContextPackRequest, ContextDocumentRef, KnowledgeSearchFilters,
        KnowledgeSearchRequest, LocationActivityRequest, RelatedDocumentsRequest,
        SyncLocationRequest,
    },
    knowledge::{
        argument_value, default_data_dir, load_locations, KnowledgeClient, LocationDefinition,
    },
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::HashSet, path::PathBuf, time::Duration};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const MCP_PROTOCOL_VERSION: &str = "2025-03-26";

#[derive(Clone, Debug, PartialEq)]
struct McpToolError {
    code: &'static str,
    message: String,
}

impl McpToolError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    fn structured_content(&self) -> Value {
        json!({
            "error": {
                "code": self.code,
                "message": self.message
            }
        })
    }
}

impl From<String> for McpToolError {
    fn from(message: String) -> Self {
        Self::new("tool_execution_failed", message)
    }
}

#[derive(Clone)]
struct McpState {
    client: KnowledgeClient,
    locations: Vec<LocationDefinition>,
    allowed_ids: HashSet<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocationIdArgs {
    location_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ActivityArgs {
    location_id: String,
    #[serde(default = "default_days")]
    days: usize,
    #[serde(default = "default_limit")]
    limit: usize,
    #[serde(default)]
    path_prefix: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchArgs {
    location_ids: Vec<String>,
    query: String,
    #[serde(default)]
    filters: KnowledgeSearchFilters,
    #[serde(default = "default_limit")]
    limit: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DocumentArgs {
    location_id: String,
    relative_path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RelatedArgs {
    location_id: String,
    relative_path: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContextArgs {
    #[serde(default)]
    query: String,
    documents: Vec<ContextDocumentRef>,
    #[serde(default = "default_context_characters")]
    max_characters: usize,
    #[serde(default = "default_context_documents")]
    max_documents: usize,
}

fn default_days() -> usize {
    15
}

fn default_limit() -> usize {
    20
}

fn default_context_characters() -> usize {
    30_000
}

fn default_context_documents() -> usize {
    20
}

pub fn run_mcp_command(arguments: &[String]) -> Result<(), String> {
    let data_dir = argument_value(arguments, "--data-dir")
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(default_data_dir)?;
    let allow_all = arguments.iter().any(|argument| argument == "--allow-all");
    let requested = argument_values(arguments, "--allow");
    if !allow_all && requested.is_empty() {
        return Err(
            "MCP access needs an explicit --allow <location-id> or --allow-all.".to_string(),
        );
    }
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Could not start the MCP runtime: {error}"))?;
    runtime.block_on(run_mcp(data_dir, allow_all, requested))
}

async fn run_mcp(data_dir: PathBuf, allow_all: bool, requested: Vec<String>) -> Result<(), String> {
    let all_locations = load_locations(&data_dir)?;
    let requested = requested.into_iter().collect::<HashSet<_>>();
    let locations = all_locations
        .into_iter()
        .filter(|location| allow_all || requested.contains(&location.id))
        .collect::<Vec<_>>();
    if locations.is_empty() {
        return Err(
            "None of the allowed Location identifiers are registered in Construct.".to_string(),
        );
    }
    if !allow_all {
        let found = locations
            .iter()
            .map(|location| location.id.as_str())
            .collect::<HashSet<_>>();
        if let Some(missing) = requested.iter().find(|id| !found.contains(id.as_str())) {
            return Err(format!(
                "The allowed Location `{missing}` is not registered in Construct."
            ));
        }
    }
    let client = KnowledgeClient::new(data_dir)?;
    for location in &locations {
        let _ = sync_location(&client, location).await;
    }
    let state = McpState {
        client: client.clone(),
        allowed_ids: locations
            .iter()
            .map(|location| location.id.clone())
            .collect(),
        locations: locations.clone(),
    };
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            for location in &locations {
                let _ = sync_location(&client, location).await;
            }
        }
    });

    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();
    let mut stdout = tokio::io::stdout();
    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|error| format!("Could not read MCP stdin: {error}"))?
    {
        if line.trim().is_empty() {
            continue;
        }
        let message: Value = match serde_json::from_str(&line) {
            Ok(message) => message,
            Err(error) => {
                write_message(
                    &mut stdout,
                    jsonrpc_error(Value::Null, -32700, &format!("Parse error: {error}")),
                )
                .await?;
                continue;
            }
        };
        let id = message.get("id").cloned();
        if id.is_none() {
            continue;
        }
        let response = handle_message(&state, message).await;
        write_message(&mut stdout, response).await?;
    }
    Ok(())
}

async fn sync_location(
    client: &KnowledgeClient,
    location: &LocationDefinition,
) -> Result<(), String> {
    client
        .sync(SyncLocationRequest {
            location_id: location.id.clone(),
            root_path: location.path.clone(),
            display_name: location.name.clone(),
            okf_bundle: location.okf_bundle,
            rebuild: false,
        })
        .await
        .map(|_| ())
}

async fn handle_message(state: &McpState, message: Value) -> Value {
    let id = message.get("id").cloned().unwrap_or(Value::Null);
    let method = message
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match method {
        "initialize" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": "Construct", "version": env!("CARGO_PKG_VERSION") },
                "instructions": "Read-only local access to explicitly allowed Construct Locations. Use overview before broad search."
            }
        }),
        "ping" => json!({ "jsonrpc": "2.0", "id": id, "result": {} }),
        "tools/list" => json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "tools": tool_definitions() }
        }),
        "tools/call" => {
            let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let arguments = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            match call_tool(state, name, arguments).await {
                Ok(result) => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{ "type": "text", "text": serde_json::to_string_pretty(&result).unwrap_or_default() }],
                        "structuredContent": result,
                        "isError": false
                    }
                }),
                Err(error) => {
                    json!({ "jsonrpc": "2.0", "id": id, "result": tool_error_result(error) })
                }
            }
        }
        _ => jsonrpc_error(id, -32601, "Method not found"),
    }
}

async fn call_tool(state: &McpState, name: &str, arguments: Value) -> Result<Value, McpToolError> {
    match name {
        "construct_list_locations" => {
            let mut output = Vec::new();
            for location in &state.locations {
                let status = state.client.status(&location.id).await?;
                output.push(json!({
                    "id": location.id,
                    "name": location.name,
                    "okfBundle": location.okf_bundle,
                    "index": status,
                    "capabilities": ["overview", "activity", "search", "read", "related", "context"]
                }));
            }
            Ok(json!({ "locations": output }))
        }
        "construct_get_location_overview" => {
            let args: LocationIdArgs = decode(arguments)?;
            ensure_allowed(state, &args.location_id)?;
            encode(state.client.location_overview(&args.location_id).await?)
        }
        "construct_get_location_activity" => {
            let args: ActivityArgs = decode(arguments)?;
            ensure_allowed(state, &args.location_id)?;
            encode(
                state
                    .client
                    .location_activity(LocationActivityRequest {
                        location_id: args.location_id,
                        days: args.days,
                        limit: args.limit,
                        path_prefix: args.path_prefix,
                    })
                    .await?,
            )
        }
        "construct_search_knowledge" => {
            let args: SearchArgs = decode(arguments)?;
            ensure_allowed_many(state, &args.location_ids)?;
            encode(
                state
                    .client
                    .search_knowledge(KnowledgeSearchRequest {
                        location_ids: args.location_ids,
                        query: args.query,
                        filters: args.filters,
                        limit: args.limit,
                    })
                    .await?,
            )
        }
        "construct_read_document" => {
            let args: DocumentArgs = decode(arguments)?;
            ensure_allowed(state, &args.location_id)?;
            encode(
                state
                    .client
                    .get_document(&args.location_id, &args.relative_path, true)
                    .await?
                    .ok_or_else(|| {
                        McpToolError::new(
                            "document_not_found",
                            "The document was not found in the active index.",
                        )
                    })?,
            )
        }
        "construct_get_related_documents" => {
            let args: RelatedArgs = decode(arguments)?;
            ensure_allowed(state, &args.location_id)?;
            encode(
                state
                    .client
                    .related_documents(RelatedDocumentsRequest {
                        location_id: args.location_id,
                        relative_path: args.relative_path,
                        limit: args.limit,
                    })
                    .await?,
            )
        }
        "construct_build_context_pack" => {
            let args: ContextArgs = decode(arguments)?;
            let ids = args
                .documents
                .iter()
                .map(|document| document.location_id.clone())
                .collect::<Vec<_>>();
            ensure_allowed_many(state, &ids)?;
            encode(
                state
                    .client
                    .build_context_pack(
                        BuildContextPackRequest {
                            query: args.query,
                            documents: args.documents,
                            max_characters: args.max_characters,
                            max_documents: args.max_documents,
                        },
                        true,
                    )
                    .await?,
            )
        }
        "construct_get_index_status" => {
            let args: LocationIdArgs = decode(arguments)?;
            ensure_allowed(state, &args.location_id)?;
            encode(state.client.status(&args.location_id).await?)
        }
        _ => Err(McpToolError::new("unknown_tool", "Unknown Construct tool.")),
    }
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "construct_list_locations",
            "description": "List explicitly allowed Construct Locations and their current index capabilities.",
            "inputSchema": { "type": "object", "properties": {}, "additionalProperties": false }
        },
        {
            "name": "construct_get_location_overview",
            "description": "Start here for hot memory: counts by type/tag/role, link health, recent OKF log entries, and the most active documents.",
            "inputSchema": location_schema()
        },
        {
            "name": "construct_get_location_activity",
            "description": "Return bounded 1-15 day document activity with separate changed, served, and context counts.",
            "inputSchema": {
                "type": "object",
                "required": ["locationId"],
                "properties": {
                    "locationId": { "type": "string" },
                    "days": { "type": "integer", "minimum": 1, "maximum": 15, "default": 15 },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 50, "default": 20 },
                    "pathPrefix": { "type": "string", "default": "" }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "construct_search_knowledge",
            "description": "Search saved Markdown across allowed Locations with weighted full-text ranking and OKF metadata filters.",
            "inputSchema": {
                "type": "object",
                "required": ["locationIds", "query"],
                "properties": {
                    "locationIds": { "type": "array", "items": { "type": "string" }, "minItems": 1, "maxItems": 100 },
                    "query": { "type": "string", "minLength": 1, "maxLength": 1000 },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 100, "default": 20 },
                    "filters": {
                        "type": "object",
                        "properties": {
                            "types": string_array(),
                            "tags": string_array(),
                            "roles": string_array(),
                            "statuses": string_array(),
                            "trust": string_array(),
                            "freshness": string_array(),
                            "pathPrefix": { "type": "string" },
                            "findings": { "type": "string", "enum": ["any", "with", "without"] }
                        },
                        "additionalProperties": false
                    }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "construct_read_document",
            "description": "Read one saved indexed Markdown document by Location ID and relative path. Successful reads contribute to hot-memory activity.",
            "inputSchema": document_schema()
        },
        {
            "name": "construct_get_related_documents",
            "description": "Return bounded incoming and outgoing Markdown links for one indexed document.",
            "inputSchema": {
                "type": "object",
                "required": ["locationId", "relativePath"],
                "properties": {
                    "locationId": { "type": "string" },
                    "relativePath": { "type": "string" },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 50, "default": 20 }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "construct_build_context_pack",
            "description": "Assemble a bounded, provenance-rich context pack from explicit indexed documents. Included documents contribute to hot-memory activity.",
            "inputSchema": {
                "type": "object",
                "required": ["documents"],
                "properties": {
                    "query": { "type": "string", "maxLength": 1000 },
                    "documents": {
                        "type": "array",
                        "minItems": 1,
                        "maxItems": 100,
                        "items": {
                            "type": "object",
                            "required": ["locationId", "relativePath"],
                            "properties": {
                                "locationId": { "type": "string" },
                                "relativePath": { "type": "string" },
                                "reason": { "type": "string" }
                            },
                            "additionalProperties": false
                        }
                    },
                    "maxCharacters": { "type": "integer", "minimum": 1000, "maximum": 200000, "default": 30000 },
                    "maxDocuments": { "type": "integer", "minimum": 1, "maximum": 20, "default": 20 }
                },
                "additionalProperties": false
            }
        },
        {
            "name": "construct_get_index_status",
            "description": "Return the active index generation, freshness, counts, storage size, and error state for one allowed Location.",
            "inputSchema": location_schema()
        }
    ])
}

fn location_schema() -> Value {
    json!({
        "type": "object",
        "required": ["locationId"],
        "properties": { "locationId": { "type": "string" } },
        "additionalProperties": false
    })
}

fn document_schema() -> Value {
    json!({
        "type": "object",
        "required": ["locationId", "relativePath"],
        "properties": {
            "locationId": { "type": "string" },
            "relativePath": { "type": "string" }
        },
        "additionalProperties": false
    })
}

fn string_array() -> Value {
    json!({ "type": "array", "items": { "type": "string" } })
}

fn ensure_allowed(state: &McpState, location_id: &str) -> Result<(), McpToolError> {
    if state.allowed_ids.contains(location_id) {
        Ok(())
    } else {
        Err(McpToolError::new(
            "location_not_allowed",
            "This Location is not in the MCP allowlist.",
        ))
    }
}

fn ensure_allowed_many(state: &McpState, location_ids: &[String]) -> Result<(), McpToolError> {
    if location_ids.is_empty() {
        return Err(McpToolError::new(
            "location_required",
            "Choose at least one allowed Location.",
        ));
    }
    for location_id in location_ids {
        ensure_allowed(state, location_id)?;
    }
    Ok(())
}

fn argument_values(arguments: &[String], flag: &str) -> Vec<String> {
    arguments
        .windows(2)
        .filter(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
        .collect()
}

fn decode<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, McpToolError> {
    serde_json::from_value(value).map_err(|error| {
        McpToolError::new(
            "invalid_arguments",
            format!("Invalid tool arguments: {error}"),
        )
    })
}

fn encode<T: serde::Serialize>(value: T) -> Result<Value, McpToolError> {
    serde_json::to_value(value).map_err(|error| {
        McpToolError::new(
            "result_encoding_failed",
            format!("Could not encode the tool result: {error}"),
        )
    })
}

fn tool_error_result(error: McpToolError) -> Value {
    json!({
        "content": [{ "type": "text", "text": error.message }],
        "structuredContent": error.structured_content(),
        "isError": true
    })
}

fn jsonrpc_error(id: Value, code: i32, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    })
}

async fn write_message(stdout: &mut tokio::io::Stdout, message: Value) -> Result<(), String> {
    let mut encoded = serde_json::to_vec(&message)
        .map_err(|error| format!("Could not encode MCP output: {error}"))?;
    encoded.push(b'\n');
    stdout
        .write_all(&encoded)
        .await
        .map_err(|error| format!("Could not write MCP stdout: {error}"))?;
    stdout
        .flush()
        .await
        .map_err(|error| format!("Could not flush MCP stdout: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_errors_include_a_stable_structured_code() {
        let result = tool_error_result(McpToolError::new(
            "location_not_allowed",
            "This Location is not in the MCP allowlist.",
        ));

        assert_eq!(result["isError"], true);
        assert_eq!(
            result["structuredContent"]["error"]["code"],
            "location_not_allowed"
        );
        assert_eq!(
            result["structuredContent"]["error"]["message"],
            "This Location is not in the MCP allowlist."
        );
        assert_eq!(
            result["content"][0]["text"],
            "This Location is not in the MCP allowlist."
        );
    }
}
