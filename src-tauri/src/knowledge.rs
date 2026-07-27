use crate::index::{
    self, ActivityKind, BuildContextPackRequest, ContextPackResponse, IndexStatus,
    IndexedDocumentView, KnowledgeSearchRequest, KnowledgeSearchResponse, LocationActivityRequest,
    LocationActivityResponse, LocationOverview, RelatedDocumentsRequest, RelatedDocumentsResponse,
    SearchFacets, SearchFacetsRequest, SearchIndexRequest, SearchResult, SyncLocationRequest,
};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

#[cfg(unix)]
use std::{io::ErrorKind, os::unix::fs::PermissionsExt};
#[cfg(any(unix, windows))]
use tokio::io::{split, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
#[cfg(windows)]
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient, ServerOptions};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};

const PROTOCOL_VERSION: u32 = 1;
const MAX_MESSAGE_BYTES: usize = 12 * 1024 * 1024;
const SOCKET_NAME: &str = "knowledge-service.sock";
const TOKEN_NAME: &str = "knowledge-service.token";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LocationDefinition {
    pub(crate) id: String,
    pub(crate) path: String,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) available: bool,
    #[serde(default)]
    pub(crate) okf_bundle: bool,
}

#[derive(Default, Deserialize)]
struct WorkspaceLocations {
    #[serde(default)]
    locations: Vec<LocationDefinition>,
}

#[derive(Clone)]
pub(crate) struct KnowledgeClient {
    data_dir: PathBuf,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IpcRequest {
    protocol_version: u32,
    token: String,
    operation: String,
    payload: Value,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IpcResponse {
    protocol_version: u32,
    result: Option<Value>,
    error: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReadDocumentRequest {
    location_id: String,
    relative_path: String,
    #[serde(default)]
    track_activity: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContextRequest {
    request: BuildContextPackRequest,
    #[serde(default)]
    track_activity: bool,
}

impl KnowledgeClient {
    pub(crate) fn new(data_dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&data_dir).map_err(|error| {
            format!("Could not create Construct's local data directory: {error}")
        })?;
        Ok(Self { data_dir })
    }

    pub(crate) fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub(crate) async fn sync(&self, request: SyncLocationRequest) -> Result<IndexStatus, String> {
        self.call("sync", request).await
    }

    pub(crate) async fn status(&self, location_id: &str) -> Result<IndexStatus, String> {
        self.call("status", json!({ "locationId": location_id }))
            .await
    }

    pub(crate) async fn search(
        &self,
        request: SearchIndexRequest,
    ) -> Result<Vec<SearchResult>, String> {
        self.call("search", request).await
    }

    pub(crate) async fn search_knowledge(
        &self,
        request: KnowledgeSearchRequest,
    ) -> Result<KnowledgeSearchResponse, String> {
        self.call("searchKnowledge", request).await
    }

    pub(crate) async fn search_facets(
        &self,
        request: SearchFacetsRequest,
    ) -> Result<SearchFacets, String> {
        self.call("searchFacets", request).await
    }

    pub(crate) async fn get_document(
        &self,
        location_id: &str,
        relative_path: &str,
        track_activity: bool,
    ) -> Result<Option<IndexedDocumentView>, String> {
        self.call(
            "getDocument",
            ReadDocumentRequest {
                location_id: location_id.to_string(),
                relative_path: relative_path.to_string(),
                track_activity,
            },
        )
        .await
    }

    pub(crate) async fn related_documents(
        &self,
        request: RelatedDocumentsRequest,
    ) -> Result<RelatedDocumentsResponse, String> {
        self.call("relatedDocuments", request).await
    }

    pub(crate) async fn build_context_pack(
        &self,
        request: BuildContextPackRequest,
        track_activity: bool,
    ) -> Result<ContextPackResponse, String> {
        self.call(
            "buildContextPack",
            ContextRequest {
                request,
                track_activity,
            },
        )
        .await
    }

    pub(crate) async fn location_overview(
        &self,
        location_id: &str,
    ) -> Result<LocationOverview, String> {
        self.call("locationOverview", json!({ "locationId": location_id }))
            .await
    }

    pub(crate) async fn location_activity(
        &self,
        request: LocationActivityRequest,
    ) -> Result<LocationActivityResponse, String> {
        self.call("locationActivity", request).await
    }

    pub(crate) async fn delete(&self, location_id: &str) -> Result<(), String> {
        self.call("delete", json!({ "locationId": location_id }))
            .await
    }

    #[cfg(any(unix, windows))]
    async fn call<T: Serialize, R: DeserializeOwned>(
        &self,
        operation: &str,
        payload: T,
    ) -> Result<R, String> {
        let token = ensure_token(&self.data_dir)?;
        let request = IpcRequest {
            protocol_version: PROTOCOL_VERSION,
            token,
            operation: operation.to_string(),
            payload: serde_json::to_value(payload)
                .map_err(|error| format!("Could not encode the local request: {error}"))?,
        };
        let mut stream = match connect(&self.data_dir).await {
            Ok(stream) => stream,
            Err(_) => {
                self.start_service()?;
                connect_with_retry(&self.data_dir).await?
            }
        };
        let mut encoded = serde_json::to_vec(&request)
            .map_err(|error| format!("Could not encode the local request: {error}"))?;
        if encoded.len() > MAX_MESSAGE_BYTES {
            return Err("The local request is too large.".to_string());
        }
        encoded.push(b'\n');
        stream
            .write_all(&encoded)
            .await
            .map_err(|error| format!("Could not send the local request: {error}"))?;
        let mut reader = BufReader::new(stream);
        let mut response = Vec::new();
        reader
            .read_until(b'\n', &mut response)
            .await
            .map_err(|error| format!("Could not read the local response: {error}"))?;
        if response.len() > MAX_MESSAGE_BYTES {
            return Err("The local response is too large.".to_string());
        }
        let response: IpcResponse = serde_json::from_slice(&response)
            .map_err(|error| format!("Could not decode the local response: {error}"))?;
        if response.protocol_version != PROTOCOL_VERSION {
            return Err("Construct's local service uses an incompatible protocol.".to_string());
        }
        if let Some(error) = response.error {
            return Err(error);
        }
        serde_json::from_value(response.result.unwrap_or(Value::Null))
            .map_err(|error| format!("Could not decode the local result: {error}"))
    }

    #[cfg(not(any(unix, windows)))]
    async fn call<T: Serialize, R: DeserializeOwned>(
        &self,
        _operation: &str,
        _payload: T,
    ) -> Result<R, String> {
        Err(
            "Local knowledge indexing and agent access are not available on this operating system."
                .to_string(),
        )
    }

    #[cfg(any(unix, windows))]
    fn start_service(&self) -> Result<(), String> {
        let executable = std::env::current_exe()
            .map_err(|error| format!("Could not locate the Construct executable: {error}"))?;
        std::process::Command::new(executable)
            .arg("service")
            .arg("--data-dir")
            .arg(&self.data_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("Could not start Construct's local service: {error}"))?;
        Ok(())
    }
}

pub(crate) fn default_data_dir() -> Result<PathBuf, String> {
    dirs::data_dir()
        .map(|path| path.join("com.luisnovo.construct"))
        .ok_or_else(|| "Could not locate the operating system data directory.".to_string())
}

pub(crate) fn load_locations(data_dir: &Path) -> Result<Vec<LocationDefinition>, String> {
    let path = data_dir.join("workspace.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let contents = fs::read_to_string(path)
        .map_err(|error| format!("Could not read Construct's registered Locations: {error}"))?;
    let workspace: WorkspaceLocations = serde_json::from_str(&contents)
        .map_err(|error| format!("Could not decode Construct's registered Locations: {error}"))?;
    Ok(workspace.locations)
}

pub(crate) fn mcp_configuration(data_dir: &Path, location_id: &str) -> Result<String, String> {
    let executable = std::env::current_exe()
        .map_err(|error| format!("Could not locate the Construct executable: {error}"))?;
    serde_json::to_string_pretty(&json!({
        "mcpServers": {
            "construct": {
                "command": executable,
                "args": [
                    "mcp",
                    "serve",
                    "--data-dir",
                    data_dir,
                    "--allow",
                    location_id
                ]
            }
        }
    }))
    .map_err(|error| format!("Could not create the MCP configuration: {error}"))
}

pub fn run_service_command(arguments: &[String]) -> Result<(), String> {
    let data_dir = argument_value(arguments, "--data-dir")
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(default_data_dir)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Could not start Construct's local runtime: {error}"))?;
    runtime.block_on(run_service(data_dir))
}

pub(crate) fn argument_value(arguments: &[String], flag: &str) -> Option<String> {
    arguments
        .windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
}

#[cfg(unix)]
type LocalStream = UnixStream;
#[cfg(windows)]
type LocalStream = NamedPipeClient;

#[cfg(unix)]
async fn connect(data_dir: &Path) -> Result<LocalStream, String> {
    UnixStream::connect(data_dir.join(SOCKET_NAME))
        .await
        .map_err(|error| format!("Could not connect to Construct's local service: {error}"))
}

#[cfg(windows)]
async fn connect(data_dir: &Path) -> Result<LocalStream, String> {
    ClientOptions::new()
        .open(pipe_name(data_dir))
        .map_err(|error| format!("Could not connect to Construct's local service: {error}"))
}

#[cfg(any(unix, windows))]
async fn connect_with_retry(data_dir: &Path) -> Result<LocalStream, String> {
    let mut last_error = String::new();
    for _ in 0..40 {
        match connect(data_dir).await {
            Ok(stream) => return Ok(stream),
            Err(error) => last_error = error,
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err(if last_error.is_empty() {
        "Construct's local service did not become available.".to_string()
    } else {
        last_error
    })
}

#[cfg(windows)]
fn pipe_name(data_dir: &Path) -> String {
    let identity = data_dir.to_string_lossy().replace('\\', "/").to_lowercase();
    let digest = blake3::hash(identity.as_bytes()).to_hex().to_string();
    format!(r"\\.\pipe\construct-knowledge-{}", &digest[..24])
}

fn ensure_token(data_dir: &Path) -> Result<String, String> {
    let path = data_dir.join(TOKEN_NAME);
    if path.exists() {
        return fs::read_to_string(path)
            .map(|value| value.trim().to_string())
            .map_err(|error| format!("Could not read the local service token: {error}"));
    }
    let token = format!("{}{}", uuid::Uuid::new_v4(), uuid::Uuid::new_v4());
    fs::write(&path, &token)
        .map_err(|error| format!("Could not create the local service token: {error}"))?;
    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("Could not protect the local service token: {error}"))?;
    Ok(token)
}

#[cfg(unix)]
async fn run_service(data_dir: PathBuf) -> Result<(), String> {
    fs::create_dir_all(&data_dir)
        .map_err(|error| format!("Could not create Construct's local data directory: {error}"))?;
    let token = ensure_token(&data_dir)?;
    let socket_path = data_dir.join(SOCKET_NAME);
    if UnixStream::connect(&socket_path).await.is_ok() {
        return Ok(());
    }
    match fs::remove_file(&socket_path) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => {
            return Err(format!(
                "Could not recover the local service socket: {error}"
            ))
        }
    }
    let listener = UnixListener::bind(&socket_path)
        .map_err(|error| format!("Could not bind Construct's local service: {error}"))?;
    fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("Could not protect the local service socket: {error}"))?;
    let service = index::IndexService::new(data_dir.join("indexes"))?;
    loop {
        let (stream, _) = listener.accept().await.map_err(|error| {
            format!("Construct's local service stopped accepting requests: {error}")
        })?;
        let service = service.clone();
        let token = token.clone();
        tokio::spawn(async move {
            let _ = handle_connection(stream, service, token).await;
        });
    }
}

#[cfg(windows)]
async fn run_service(data_dir: PathBuf) -> Result<(), String> {
    fs::create_dir_all(&data_dir)
        .map_err(|error| format!("Could not create Construct's local data directory: {error}"))?;
    let token = ensure_token(&data_dir)?;
    if connect(&data_dir).await.is_ok() {
        return Ok(());
    }

    let name = pipe_name(&data_dir);
    let service = index::IndexService::new(data_dir.join("indexes"))?;
    let mut first_instance = true;
    loop {
        let server = ServerOptions::new()
            .first_pipe_instance(first_instance)
            .create(&name)
            .map_err(|error| format!("Could not create Construct's local named pipe: {error}"))?;
        first_instance = false;
        server
            .connect()
            .await
            .map_err(|error| format!("Could not accept a local service request: {error}"))?;
        let service = service.clone();
        let token = token.clone();
        tokio::spawn(async move {
            let _ = handle_connection(server, service, token).await;
        });
    }
}

#[cfg(not(any(unix, windows)))]
async fn run_service(_data_dir: PathBuf) -> Result<(), String> {
    Err("Independent agent access is not available on this operating system.".to_string())
}

#[cfg(any(unix, windows))]
async fn handle_connection<S>(
    stream: S,
    service: index::IndexService,
    token: String,
) -> Result<(), String>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (reader, mut writer) = split(stream);
    let mut reader = BufReader::new(reader);
    let mut request = Vec::new();
    reader
        .read_until(b'\n', &mut request)
        .await
        .map_err(|error| format!("Could not read a local service request: {error}"))?;
    let response = if request.len() > MAX_MESSAGE_BYTES {
        error_response("The local request is too large.")
    } else {
        match serde_json::from_slice::<IpcRequest>(&request) {
            Ok(request)
                if request.protocol_version == PROTOCOL_VERSION && request.token == token =>
            {
                match dispatch(&service, request).await {
                    Ok(result) => IpcResponse {
                        protocol_version: PROTOCOL_VERSION,
                        result: Some(result),
                        error: None,
                    },
                    Err(error) => error_response(&error),
                }
            }
            Ok(_) => error_response("The local service request was not authorized."),
            Err(_) => error_response("The local service request was malformed."),
        }
    };
    let mut encoded = serde_json::to_vec(&response)
        .map_err(|error| format!("Could not encode a local service response: {error}"))?;
    encoded.push(b'\n');
    writer
        .write_all(&encoded)
        .await
        .map_err(|error| format!("Could not send a local service response: {error}"))
}

fn error_response(error: &str) -> IpcResponse {
    IpcResponse {
        protocol_version: PROTOCOL_VERSION,
        result: None,
        error: Some(error.to_string()),
    }
}

async fn dispatch(service: &index::IndexService, request: IpcRequest) -> Result<Value, String> {
    match request.operation.as_str() {
        "sync" => {
            let request: SyncLocationRequest = decode(request.payload)?;
            let root = PathBuf::from(&request.root_path);
            encode(service.sync(request, root).await?)
        }
        "status" => {
            let location_id = required_string(&request.payload, "locationId")?;
            encode(service.status(location_id).await?)
        }
        "search" => encode(
            service
                .search(decode::<SearchIndexRequest>(request.payload)?)
                .await?,
        ),
        "searchKnowledge" => encode(
            service
                .search_knowledge(decode::<KnowledgeSearchRequest>(request.payload)?)
                .await?,
        ),
        "searchFacets" => encode(
            service
                .search_facets(decode::<SearchFacetsRequest>(request.payload)?)
                .await?,
        ),
        "getDocument" => {
            let request: ReadDocumentRequest = decode(request.payload)?;
            let document = service
                .get_document(&request.location_id, &request.relative_path)
                .await?;
            if document.is_some() && request.track_activity {
                service
                    .record_document_activity(
                        &request.location_id,
                        &request.relative_path,
                        ActivityKind::Served,
                    )
                    .await?;
            }
            encode(document)
        }
        "relatedDocuments" => encode(
            service
                .related_documents(decode::<RelatedDocumentsRequest>(request.payload)?)
                .await?,
        ),
        "buildContextPack" => {
            let request: ContextRequest = decode(request.payload)?;
            let response = service.build_context_pack(request.request).await?;
            if request.track_activity {
                for item in &response.items {
                    service
                        .record_document_activity(
                            &item.location_id,
                            &item.relative_path,
                            ActivityKind::Context,
                        )
                        .await?;
                }
            }
            encode(response)
        }
        "locationOverview" => {
            let location_id = required_string(&request.payload, "locationId")?;
            encode(service.location_overview(location_id).await?)
        }
        "locationActivity" => encode(
            service
                .location_activity(decode::<LocationActivityRequest>(request.payload)?)
                .await?,
        ),
        "delete" => {
            let location_id = required_string(&request.payload, "locationId")?;
            service.delete(location_id).await?;
            Ok(Value::Null)
        }
        _ => Err("The local service operation is not supported.".to_string()),
    }
}

fn decode<T: DeserializeOwned>(value: Value) -> Result<T, String> {
    serde_json::from_value(value)
        .map_err(|error| format!("Could not decode the local service request: {error}"))
}

fn encode<T: Serialize>(value: T) -> Result<Value, String> {
    serde_json::to_value(value)
        .map_err(|error| format!("Could not encode the local service result: {error}"))
}

fn required_string<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("The local service request is missing {key}."))
}
