use crate::{
    diagnostics::Diagnostics,
    index::{
        self, ActivityKind, BuildContextPackRequest, ContextPackResponse, IndexStatus,
        IndexedDocumentView, KnowledgeSearchRequest, KnowledgeSearchResponse, ListDocumentsRequest,
        ListDocumentsResponse, LocationActivityRequest, LocationActivityResponse, LocationOverview,
        RelatedDocumentsRequest, RelatedDocumentsResponse, SearchFacets, SearchFacetsRequest,
        SearchIndexRequest, SearchResult, SyncLocationRequest,
    },
};
use fs2::FileExt;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex as StdMutex, Weak},
    time::{Duration, Instant},
};

#[cfg(unix)]
use std::{io::ErrorKind, os::unix::fs::PermissionsExt};
#[cfg(any(unix, windows))]
use tokio::io::{split, AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
#[cfg(windows)]
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient, ServerOptions};
#[cfg(unix)]
use tokio::net::{UnixListener, UnixStream};
#[cfg(any(unix, windows))]
use tokio::sync::Notify;
#[cfg(any(unix, windows))]
use tokio::task::JoinSet;

const PROTOCOL_VERSION: u32 = 1;
const MAX_MESSAGE_BYTES: usize = 12 * 1024 * 1024;
const SOCKET_NAME: &str = "knowledge-service.sock";
const TOKEN_NAME: &str = "knowledge-service.token";
const LOCK_NAME: &str = "knowledge-service.lock";
const SERVICE_IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const SERVICE_DRAIN_TIMEOUT: Duration = Duration::from_secs(10);
const INDEX_RELEASE_TIMEOUT: Duration = Duration::from_secs(1);
const SERVICE_SHUTTING_DOWN_ERROR: &str = "Construct's local service is shutting down.";

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
    diagnostics: Diagnostics,
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
        let diagnostics = Diagnostics::new(data_dir.clone(), "construct");
        diagnostics.info("knowledge_client_ready", json!({}));
        Ok(Self {
            data_dir,
            diagnostics,
        })
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

    pub(crate) async fn list_documents(
        &self,
        request: ListDocumentsRequest,
    ) -> Result<ListDocumentsResponse, String> {
        self.call("listDocuments", request).await
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
        let mut encoded = serde_json::to_vec(&request)
            .map_err(|error| format!("Could not encode the local request: {error}"))?;
        if encoded.len() > MAX_MESSAGE_BYTES {
            return Err("The local request is too large.".to_string());
        }
        encoded.push(b'\n');
        let response = match connect(&self.data_dir).await {
            Ok(stream) => send_retryable_request(stream, &encoded).await,
            Err(error) => Err(error),
        };
        let response = match response {
            Ok(response) => response,
            Err(mut error) => {
                let mut recovered = None;
                for attempt in 1..=2 {
                    self.diagnostics.warn(
                        "knowledge_service_client_restart_requested",
                        json!({
                            "operation": operation,
                            "attempt": attempt,
                            "errorKind": error_category(&error)
                        }),
                    );
                    self.start_service()?;
                    match connect_with_retry(&self.data_dir).await {
                        Ok(stream) => match send_retryable_request(stream, &encoded).await {
                            Ok(response) => {
                                recovered = Some(response);
                                break;
                            }
                            Err(retry_error) => error = retry_error,
                        },
                        Err(retry_error) => error = retry_error,
                    }
                }
                recovered.ok_or(error)?
            }
        };
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
        self.diagnostics
            .info("knowledge_service_start_requested", json!({}));
        let result = std::process::Command::new(executable)
            .arg("service")
            .arg("--data-dir")
            .arg(&self.data_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        match result {
            Ok(_) => Ok(()),
            Err(error) => {
                self.diagnostics.error(
                    "knowledge_service_start_failed",
                    &error.to_string(),
                    json!({}),
                );
                Err(format!(
                    "Could not start Construct's local service: {error}"
                ))
            }
        }
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

pub(crate) fn mcp_configuration(
    data_dir: &Path,
    location_ids: &[String],
    allow_all: bool,
) -> Result<String, String> {
    if allow_all && !location_ids.is_empty() {
        return Err(
            "Choose specific Locations or all Locations, but not both at once.".to_string(),
        );
    }
    if !allow_all && location_ids.is_empty() {
        return Err("Choose at least one Location for MCP access.".to_string());
    }
    let executable = std::env::current_exe()
        .map_err(|error| format!("Could not locate the Construct executable: {error}"))?;
    let mut arguments = vec![
        "mcp".to_string(),
        "serve".to_string(),
        "--data-dir".to_string(),
        data_dir.to_string_lossy().into_owned(),
    ];
    if allow_all {
        arguments.push("--allow-all".to_string());
    } else {
        for location_id in location_ids {
            arguments.push("--allow".to_string());
            arguments.push(location_id.clone());
        }
    }
    serde_json::to_string_pretty(&json!({
        "mcpServers": {
            "construct": {
                "command": executable,
                "args": arguments
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
    let diagnostics = Diagnostics::new(data_dir.clone(), "knowledge-service");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Could not start Construct's local runtime: {error}"))?;
    let result = runtime.block_on(run_service(data_dir));
    if let Err(error) = &result {
        diagnostics.error("service_stopped", error, json!({}));
    }
    result
}

#[cfg(all(test, any(unix, windows)))]
pub(crate) async fn run_test_service(
    data_dir: PathBuf,
    idle_timeout: Duration,
    drain_timeout: Duration,
) -> Result<(), String> {
    run_service_with_config(
        data_dir,
        ServiceConfig {
            idle_timeout,
            drain_timeout,
        },
    )
    .await
}

pub(crate) fn argument_value(arguments: &[String], flag: &str) -> Option<String> {
    arguments
        .windows(2)
        .find(|pair| pair[0] == flag)
        .map(|pair| pair[1].clone())
}

fn error_category(error: &str) -> &'static str {
    if error.contains("connect") || error.contains("pipe") || error.contains("socket") {
        "connection"
    } else if error.contains("protocol") {
        "protocol"
    } else {
        "unavailable"
    }
}

#[cfg(any(unix, windows))]
#[derive(Clone, Copy)]
struct ServiceConfig {
    idle_timeout: Duration,
    drain_timeout: Duration,
}

#[cfg(any(unix, windows))]
impl Default for ServiceConfig {
    fn default() -> Self {
        Self {
            idle_timeout: SERVICE_IDLE_TIMEOUT,
            drain_timeout: SERVICE_DRAIN_TIMEOUT,
        }
    }
}

#[cfg(any(unix, windows))]
#[derive(Clone)]
struct ServiceActivity {
    inner: Arc<ServiceActivityInner>,
}

#[cfg(any(unix, windows))]
struct ServiceActivityInner {
    state: StdMutex<ServiceActivityState>,
    changed: Notify,
}

#[cfg(any(unix, windows))]
struct ServiceActivityState {
    last_activity: Instant,
    in_flight: usize,
    accepting: bool,
}

#[cfg(any(unix, windows))]
struct RequestActivity {
    activity: ServiceActivity,
}

#[cfg(any(unix, windows))]
impl ServiceActivity {
    fn new() -> Self {
        Self {
            inner: Arc::new(ServiceActivityInner {
                state: StdMutex::new(ServiceActivityState {
                    last_activity: Instant::now(),
                    in_flight: 0,
                    accepting: true,
                }),
                changed: Notify::new(),
            }),
        }
    }

    fn accept_authenticated(&self) -> Option<RequestActivity> {
        let mut state = self.inner.state.lock().expect("service activity lock");
        if !state.accepting {
            return None;
        }
        state.last_activity = Instant::now();
        state.in_flight += 1;
        drop(state);
        self.inner.changed.notify_waiters();
        Some(RequestActivity {
            activity: self.clone(),
        })
    }

    fn stop_accepting(&self) -> usize {
        let mut state = self.inner.state.lock().expect("service activity lock");
        state.accepting = false;
        let in_flight = state.in_flight;
        drop(state);
        self.inner.changed.notify_waiters();
        in_flight
    }

    fn in_flight(&self) -> usize {
        self.inner
            .state
            .lock()
            .expect("service activity lock")
            .in_flight
    }

    async fn wait_for_idle(&self, timeout: Duration) {
        loop {
            let notified = self.inner.changed.notified();
            let deadline = {
                let state = self.inner.state.lock().expect("service activity lock");
                if state.in_flight > 0 {
                    None
                } else {
                    Some(state.last_activity + timeout)
                }
            };
            let Some(deadline) = deadline else {
                notified.await;
                continue;
            };
            tokio::select! {
                _ = tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)) => {
                    let state = self.inner.state.lock().expect("service activity lock");
                    if state.in_flight == 0 && Instant::now() >= state.last_activity + timeout {
                        return;
                    }
                }
                _ = notified => {}
            }
        }
    }

    async fn wait_until_drained(&self) {
        loop {
            let notified = self.inner.changed.notified();
            if self.in_flight() == 0 {
                return;
            }
            notified.await;
        }
    }
}

#[cfg(any(unix, windows))]
impl Drop for RequestActivity {
    fn drop(&mut self) {
        let mut state = self
            .activity
            .inner
            .state
            .lock()
            .expect("service activity lock");
        state.in_flight = state.in_flight.saturating_sub(1);
        state.last_activity = Instant::now();
        drop(state);
        self.activity.inner.changed.notify_waiters();
    }
}

#[cfg(any(unix, windows))]
#[derive(Clone, Copy)]
enum ShutdownReason {
    Idle,
    Signal(&'static str),
}

#[cfg(any(unix, windows))]
impl ShutdownReason {
    fn diagnostic_value(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Signal(signal) => signal,
        }
    }
}

#[cfg(any(unix, windows))]
async fn send_request(stream: LocalStream, encoded: &[u8]) -> Result<IpcResponse, String> {
    let mut stream = stream;
    stream
        .write_all(encoded)
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
    serde_json::from_slice(&response)
        .map_err(|error| format!("Could not decode the local response: {error}"))
}

#[cfg(any(unix, windows))]
async fn send_retryable_request(
    stream: LocalStream,
    encoded: &[u8],
) -> Result<IpcResponse, String> {
    retryable_service_response(send_request(stream, encoded).await?)
}

#[cfg(any(unix, windows))]
fn retryable_service_response(response: IpcResponse) -> Result<IpcResponse, String> {
    if response.protocol_version == PROTOCOL_VERSION
        && response.error.as_deref() == Some(SERVICE_SHUTTING_DOWN_ERROR)
    {
        Err(SERVICE_SHUTTING_DOWN_ERROR.to_string())
    } else {
        Ok(response)
    }
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
    run_service_with_config(data_dir, ServiceConfig::default()).await
}

#[cfg(unix)]
async fn run_service_with_config(data_dir: PathBuf, config: ServiceConfig) -> Result<(), String> {
    fs::create_dir_all(&data_dir)
        .map_err(|error| format!("Could not create Construct's local data directory: {error}"))?;
    let diagnostics = Diagnostics::new(data_dir.clone(), "knowledge-service");
    diagnostics.info("service_starting", json!({ "transport": "unixSocket" }));
    let Some(_singleton_lock) = wait_for_existing_service_or_lock(&data_dir).await? else {
        diagnostics.info("service_already_running", json!({}));
        return Ok(());
    };
    let token = ensure_token(&data_dir)?;
    let socket_path = data_dir.join(SOCKET_NAME);
    if UnixStream::connect(&socket_path).await.is_ok() {
        diagnostics.info("service_already_running", json!({}));
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
    let _socket_cleanup = SocketCleanup(socket_path.clone());
    fs::set_permissions(&socket_path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("Could not protect the local service socket: {error}"))?;
    let service = Arc::new(index::IndexService::with_diagnostics(
        data_dir.join("indexes"),
        diagnostics.clone(),
    )?);
    let activity = ServiceActivity::new();
    let mut connections = JoinSet::new();
    diagnostics.info("service_ready", json!({ "transport": "unixSocket" }));
    let shutdown = wait_for_shutdown(activity.clone(), config.idle_timeout);
    tokio::pin!(shutdown);
    let reason = loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (stream, _) = accepted.map_err(|error| {
                    format!("Construct's local service stopped accepting requests: {error}")
                })?;
                let service = Arc::downgrade(&service);
                let token = token.clone();
                let activity = activity.clone();
                connections.spawn(async move {
                    let _ = handle_connection(stream, service, token, activity).await;
                });
            }
            _ = connections.join_next(), if !connections.is_empty() => {}
            reason = &mut shutdown => break reason,
        }
    };
    drop(listener);
    finish_shutdown(&activity, &diagnostics, reason, config.drain_timeout).await;
    stop_connection_tasks(&mut connections).await;
    let index_paths = service.opened_storage_paths().await;
    drop(service);
    wait_for_index_release(&index_paths).await;
    Ok(())
}

#[cfg(unix)]
struct SocketCleanup(PathBuf);

#[cfg(unix)]
impl Drop for SocketCleanup {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[cfg(windows)]
async fn run_service(data_dir: PathBuf) -> Result<(), String> {
    run_service_with_config(data_dir, ServiceConfig::default()).await
}

#[cfg(windows)]
async fn run_service_with_config(data_dir: PathBuf, config: ServiceConfig) -> Result<(), String> {
    fs::create_dir_all(&data_dir)
        .map_err(|error| format!("Could not create Construct's local data directory: {error}"))?;
    let diagnostics = Diagnostics::new(data_dir.clone(), "knowledge-service");
    diagnostics.info("service_starting", json!({ "transport": "namedPipe" }));
    let Some(_singleton_lock) = wait_for_existing_service_or_lock(&data_dir).await? else {
        diagnostics.info("service_already_running", json!({}));
        return Ok(());
    };
    let token = ensure_token(&data_dir)?;
    if connect(&data_dir).await.is_ok() {
        diagnostics.info("service_already_running", json!({}));
        return Ok(());
    }

    let name = pipe_name(&data_dir);
    let service = Arc::new(index::IndexService::with_diagnostics(
        data_dir.join("indexes"),
        diagnostics.clone(),
    )?);
    let activity = ServiceActivity::new();
    let mut connections = JoinSet::new();
    let shutdown = wait_for_shutdown(activity.clone(), config.idle_timeout);
    tokio::pin!(shutdown);
    let mut first_instance = true;
    let reason = loop {
        let creating_first_instance = first_instance;
        let server = ServerOptions::new()
            .first_pipe_instance(creating_first_instance)
            .create(&name)
            .map_err(|error| format!("Could not create Construct's local named pipe: {error}"))?;
        first_instance = false;
        if creating_first_instance {
            diagnostics.info("service_ready", json!({ "transport": "namedPipe" }));
        }
        tokio::select! {
            connected = server.connect() => {
                connected.map_err(|error| format!("Could not accept a local service request: {error}"))?;
                let service = Arc::downgrade(&service);
                let token = token.clone();
                let activity = activity.clone();
                connections.spawn(async move {
                    let _ = handle_connection(server, service, token, activity).await;
                });
            }
            _ = connections.join_next(), if !connections.is_empty() => {}
            reason = &mut shutdown => break reason,
        }
    };
    finish_shutdown(&activity, &diagnostics, reason, config.drain_timeout).await;
    stop_connection_tasks(&mut connections).await;
    let index_paths = service.opened_storage_paths().await;
    drop(service);
    wait_for_index_release(&index_paths).await;
    Ok(())
}

#[cfg(any(unix, windows))]
async fn wait_for_existing_service_or_lock(data_dir: &Path) -> Result<Option<File>, String> {
    for _ in 0..40 {
        if let Some(lock) = acquire_singleton_lock(data_dir)? {
            return Ok(Some(lock));
        }
        if connect(data_dir).await.is_ok() {
            return Ok(None);
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    Err("Construct's local service did not become available for restart.".to_string())
}

#[cfg(any(unix, windows))]
fn acquire_singleton_lock(data_dir: &Path) -> Result<Option<File>, String> {
    let path = data_dir.join(LOCK_NAME);
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .map_err(|error| format!("Could not open the local service lock: {error}"))?;
    #[cfg(unix)]
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("Could not protect the local service lock: {error}"))?;
    match FileExt::try_lock_exclusive(&file) {
        Ok(()) => Ok(Some(file)),
        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
        Err(error) => Err(format!("Could not lock the local service: {error}")),
    }
}

#[cfg(any(unix, windows))]
async fn wait_for_shutdown(activity: ServiceActivity, idle_timeout: Duration) -> ShutdownReason {
    tokio::select! {
        _ = activity.wait_for_idle(idle_timeout) => ShutdownReason::Idle,
        signal = termination_signal() => ShutdownReason::Signal(signal),
    }
}

#[cfg(unix)]
async fn termination_signal() -> &'static str {
    use tokio::signal::unix::{signal, SignalKind};

    let Ok(mut terminate) = signal(SignalKind::terminate()) else {
        return std::future::pending().await;
    };
    tokio::select! {
        _ = terminate.recv() => "sigterm",
        result = tokio::signal::ctrl_c() => {
            if result.is_ok() {
                "ctrlC"
            } else {
                std::future::pending().await
            }
        }
    }
}

#[cfg(windows)]
async fn termination_signal() -> &'static str {
    if tokio::signal::ctrl_c().await.is_ok() {
        "ctrlC"
    } else {
        std::future::pending().await
    }
}

#[cfg(any(unix, windows))]
async fn finish_shutdown(
    activity: &ServiceActivity,
    diagnostics: &Diagnostics,
    reason: ShutdownReason,
    drain_timeout: Duration,
) {
    match reason {
        ShutdownReason::Idle => diagnostics.info("service_idle_deadline_reached", json!({})),
        ShutdownReason::Signal(signal) => {
            diagnostics.info("service_termination_requested", json!({ "signal": signal }))
        }
    }
    let in_flight_at_start = activity.stop_accepting();
    match tokio::time::timeout(drain_timeout, activity.wait_until_drained()).await {
        Ok(()) => diagnostics.info(
            "service_graceful_shutdown_completed",
            json!({
                "reason": reason.diagnostic_value(),
                "inFlightAtStart": in_flight_at_start
            }),
        ),
        Err(_) => diagnostics.warn(
            "service_graceful_shutdown_timeout",
            json!({
                "reason": reason.diagnostic_value(),
                "remainingInFlight": activity.in_flight()
            }),
        ),
    }
}

#[cfg(any(unix, windows))]
async fn stop_connection_tasks(connections: &mut JoinSet<()>) {
    connections.abort_all();
    while connections.join_next().await.is_some() {}
}

#[cfg(any(unix, windows))]
async fn wait_for_index_release(paths: &[PathBuf]) {
    let deadline = Instant::now() + INDEX_RELEASE_TIMEOUT;
    loop {
        if paths.iter().all(|path| index_lock_is_available(path)) {
            return;
        }
        if Instant::now() >= deadline {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[cfg(any(unix, windows))]
fn index_lock_is_available(path: &Path) -> bool {
    let lock_path = path.join("LOCK");
    let file = match OpenOptions::new().read(true).write(true).open(lock_path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return true,
        Err(_) => return false,
    };
    FileExt::try_lock_exclusive(&file).is_ok()
}

#[cfg(not(any(unix, windows)))]
async fn run_service(_data_dir: PathBuf) -> Result<(), String> {
    Err("Independent agent access is not available on this operating system.".to_string())
}

#[cfg(any(unix, windows))]
async fn handle_connection<S>(
    stream: S,
    service: Weak<index::IndexService>,
    token: String,
    activity: ServiceActivity,
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
    let mut request_activity = None;
    let response = if request.len() > MAX_MESSAGE_BYTES {
        error_response("The local request is too large.")
    } else {
        match serde_json::from_slice::<IpcRequest>(&request) {
            Ok(request)
                if request.protocol_version == PROTOCOL_VERSION && request.token == token =>
            {
                match (activity.accept_authenticated(), service.upgrade()) {
                    (Some(accepted_activity), Some(service)) => {
                        request_activity = Some(accepted_activity);
                        match dispatch(&service, request).await {
                            Ok(result) => IpcResponse {
                                protocol_version: PROTOCOL_VERSION,
                                result: Some(result),
                                error: None,
                            },
                            Err(error) => error_response(&error),
                        }
                    }
                    _ => error_response(SERVICE_SHUTTING_DOWN_ERROR),
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
        .map_err(|error| format!("Could not send a local service response: {error}"))?;
    drop(request_activity);
    Ok(())
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
        "listDocuments" => encode(
            service
                .list_documents(decode::<ListDocumentsRequest>(request.payload)?)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_data_dir(name: &str) -> PathBuf {
        let label = name.chars().take(8).collect::<String>();
        let path = PathBuf::from("/tmp").join(format!("ck-{label}-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&path).expect("create temporary data directory");
        path
    }

    #[cfg(unix)]
    async fn wait_for_service(data_dir: &Path) {
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if connect(data_dir).await.is_ok() {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("service becomes ready");
    }

    fn configuration_arguments(configuration: &str) -> Vec<String> {
        serde_json::from_str::<Value>(configuration).unwrap()["mcpServers"]["construct"]["args"]
            .as_array()
            .unwrap()
            .iter()
            .map(|argument| argument.as_str().unwrap().to_string())
            .collect()
    }

    #[test]
    fn mcp_configuration_supports_multiple_explicit_locations() {
        let configuration = mcp_configuration(
            Path::new("/tmp/construct-profile"),
            &["first".to_string(), "second".to_string()],
            false,
        )
        .unwrap();

        assert_eq!(
            configuration_arguments(&configuration),
            [
                "mcp",
                "serve",
                "--data-dir",
                "/tmp/construct-profile",
                "--allow",
                "first",
                "--allow",
                "second",
            ]
        );
    }

    #[test]
    fn mcp_configuration_requires_an_explicit_access_scope() {
        assert!(mcp_configuration(Path::new("/tmp/construct-profile"), &[], false).is_err());
        assert!(mcp_configuration(
            Path::new("/tmp/construct-profile"),
            &["first".to_string()],
            true,
        )
        .is_err());

        let configuration =
            mcp_configuration(Path::new("/tmp/construct-profile"), &[], true).unwrap();
        assert!(configuration_arguments(&configuration).contains(&"--allow-all".to_string()));
    }

    #[test]
    fn only_shutdown_responses_are_promoted_to_retryable_failures() {
        let shutdown = retryable_service_response(error_response(SERVICE_SHUTTING_DOWN_ERROR));
        assert_eq!(shutdown.err().as_deref(), Some(SERVICE_SHUTTING_DOWN_ERROR));

        let operation_error = retryable_service_response(error_response("Synthetic failure"))
            .expect("ordinary operation errors are returned without restarting the service");
        assert_eq!(operation_error.error.as_deref(), Some("Synthetic failure"));
    }

    #[tokio::test]
    async fn activity_waits_for_in_flight_work_and_stops_accepting_new_requests() {
        let activity = ServiceActivity::new();
        let request = activity
            .accept_authenticated()
            .expect("accept authenticated request");
        assert_eq!(activity.stop_accepting(), 1);
        assert!(activity.accept_authenticated().is_none());

        let delayed = tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(30)).await;
            drop(request);
        });
        tokio::time::timeout(Duration::from_millis(200), activity.wait_until_drained())
            .await
            .expect("in-flight request drains");
        delayed.await.expect("finish delayed request");
        assert_eq!(activity.in_flight(), 0);
    }

    #[tokio::test]
    async fn activity_drain_is_bounded_when_work_does_not_finish() {
        let activity = ServiceActivity::new();
        let request = activity
            .accept_authenticated()
            .expect("accept authenticated request");
        activity.stop_accepting();

        assert!(
            tokio::time::timeout(Duration::from_millis(20), activity.wait_until_drained())
                .await
                .is_err()
        );
        drop(request);
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn repeated_reconcile_query_cycles_idle_cleanly_and_restart() {
        let data_dir = temporary_data_dir("idle-restart");
        let source_dir = temporary_data_dir("idle-restart-source");
        fs::write(source_dir.join("one.md"), "# One\nRestartable knowledge")
            .expect("write source document");
        let config = ServiceConfig {
            idle_timeout: Duration::from_millis(80),
            drain_timeout: Duration::from_millis(100),
        };

        for _ in 0..2 {
            let mut task = tokio::spawn(run_service_with_config(data_dir.clone(), config));
            tokio::select! {
                result = &mut task => panic!("service stopped before ready: {result:?}"),
                _ = wait_for_service(&data_dir) => {}
            }
            let client = KnowledgeClient::new(data_dir.clone()).expect("create client");
            client
                .sync(SyncLocationRequest {
                    location_id: "restart-location".to_string(),
                    root_path: source_dir.to_string_lossy().to_string(),
                    display_name: "Restart Location".to_string(),
                    okf_bundle: false,
                    rebuild: false,
                    minimum_reconcile_interval_ms: 0,
                })
                .await
                .expect("reconcile location");
            let results = client
                .search(SearchIndexRequest {
                    location_id: "restart-location".to_string(),
                    query: "Restartable".to_string(),
                    limit: 10,
                })
                .await
                .expect("query knowledge");
            assert_eq!(results.len(), 1);
            tokio::time::timeout(Duration::from_secs(2), task)
                .await
                .expect("service reaches idle shutdown")
                .expect("join service")
                .expect("service exits cleanly");
            assert!(!data_dir.join(SOCKET_NAME).exists());
        }

        let diagnostics = fs::read_to_string(data_dir.join("logs/knowledge-service.log"))
            .expect("read service diagnostics");
        assert!(diagnostics.contains("\"event\":\"service_idle_deadline_reached\""));
        assert!(diagnostics.contains("\"event\":\"service_graceful_shutdown_completed\""));
        fs::remove_dir_all(data_dir).expect("remove temporary data directory");
        fs::remove_dir_all(source_dir).expect("remove temporary source directory");
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn concurrent_starters_share_one_service_instance() {
        let data_dir = temporary_data_dir("concurrent-start");
        let config = ServiceConfig {
            idle_timeout: Duration::from_millis(100),
            drain_timeout: Duration::from_millis(100),
        };
        let first = tokio::spawn(run_service_with_config(data_dir.clone(), config));
        let second = tokio::spawn(run_service_with_config(data_dir.clone(), config));
        wait_for_service(&data_dir).await;

        let client = KnowledgeClient::new(data_dir.clone()).expect("create client");
        client
            .status("singleton-location")
            .await
            .expect("query singleton service");
        let (first, second) = tokio::join!(
            tokio::time::timeout(Duration::from_secs(2), first),
            tokio::time::timeout(Duration::from_secs(2), second)
        );
        first
            .expect("first starter exits")
            .expect("join first starter")
            .expect("first starter succeeds");
        second
            .expect("second starter exits")
            .expect("join second starter")
            .expect("second starter succeeds");
        assert!(!data_dir.join(SOCKET_NAME).exists());
        fs::remove_dir_all(data_dir).expect("remove temporary data directory");
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn unauthenticated_open_connection_does_not_prevent_idle_shutdown() {
        let data_dir = temporary_data_dir("unauthenticated-idle");
        let config = ServiceConfig {
            idle_timeout: Duration::from_millis(80),
            drain_timeout: Duration::from_millis(100),
        };
        let mut task = tokio::spawn(run_service_with_config(data_dir.clone(), config));
        tokio::select! {
            result = &mut task => panic!("service stopped before ready: {result:?}"),
            _ = wait_for_service(&data_dir) => {}
        }
        let connection = UnixStream::connect(data_dir.join(SOCKET_NAME))
            .await
            .expect("open unauthenticated connection");

        tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("unauthenticated connection does not reset idle deadline")
            .expect("join service")
            .expect("service exits cleanly");
        drop(connection);
        assert!(!data_dir.join(SOCKET_NAME).exists());
        fs::remove_dir_all(data_dir).expect("remove temporary data directory");
    }
}
