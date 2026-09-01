#![cfg(all(feature = "desktop", unix))]

use serde_json::{json, Value};
use std::{fs, path::PathBuf, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines},
    net::{UnixListener, UnixStream},
    process::{Child, ChildStdin, ChildStdout, Command},
    time::timeout,
};

const DEADLINE: Duration = Duration::from_secs(2);
const EXPECTED_MAX_QUEUED_TOOL_CALLS: u64 = 32;
const EXPECTED_TOOL_NAMES: [&str; 9] = [
    "construct_list_locations",
    "construct_get_location_overview",
    "construct_get_location_activity",
    "construct_search_knowledge",
    "construct_list_documents",
    "construct_read_document",
    "construct_get_related_documents",
    "construct_build_context_pack",
    "construct_get_index_status",
];

struct Adapter {
    child: Child,
    input: Option<ChildStdin>,
    output: Lines<BufReader<ChildStdout>>,
    listener: UnixListener,
    root: PathBuf,
}

impl Adapter {
    fn start(symlink: bool) -> Self {
        // Keep the Unix socket path below the macOS limit, including on CI.
        let root = PathBuf::from("/tmp").join(format!("cmcp-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).expect("create isolated profile");
        fs::write(
            root.join("workspace.json"),
            serde_json::to_vec(&json!({
                "locations": (["allowed-a", "allowed-b", "excluded"].map(|id| json!({
                    "id": id, "name": id, "path": root.join(id), "available": true
                })))
            }))
            .unwrap(),
        )
        .expect("write registered locations");
        let listener = UnixListener::bind(root.join("knowledge-service.sock"))
            .expect("bind controlled IPC service");
        let mut binary = PathBuf::from(env!("CARGO_BIN_EXE_construct"));
        if symlink {
            let link = root.join("construct-link");
            std::os::unix::fs::symlink(binary, &link).expect("link development executable");
            binary = link;
        }
        let mut child = Command::new(binary)
            .args(["mcp", "serve", "--data-dir"])
            .arg(&root)
            .args(["--allow", "allowed-a", "--allow", "allowed-b"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .expect("spawn actual MCP entrypoint");
        Self {
            input: child.stdin.take(),
            output: BufReader::new(child.stdout.take().unwrap()).lines(),
            child,
            listener,
            root,
        }
    }

    async fn send(&mut self, message: Value) {
        let mut bytes = serde_json::to_vec(&message).unwrap();
        bytes.push(b'\n');
        timeout(DEADLINE, self.input.as_mut().unwrap().write_all(&bytes))
            .await
            .expect("stdin remains writable")
            .expect("send MCP message");
    }

    async fn response(&mut self, id: u64) -> Value {
        let line = timeout(DEADLINE, self.output.next_line())
            .await
            .expect("MCP must respond without waiting for the index")
            .expect("read MCP response")
            .expect("MCP stdout remains open");
        let message: Value = serde_json::from_str(&line).expect("stdout is JSON-RPC only");
        assert_eq!(message["id"], id);
        message
    }

    async fn initialize(&mut self) {
        self.send(json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": { "protocolVersion": "2025-03-26", "capabilities": {},
                "clientInfo": { "name": "startup-regression", "version": "1" } }
        }))
        .await;
        let initialized = self.response(1).await;
        assert_eq!(initialized["result"]["serverInfo"]["name"], "Construct");
        self.send(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))
            .await;
    }

    async fn ipc(&self, operation: &str, location: &str) -> BufReader<UnixStream> {
        // Allow OS executable verification on first launch; protocol replies
        // and EOF still have the stricter two-second deadline above.
        let (stream, _) = timeout(Duration::from_secs(10), self.listener.accept())
            .await
            .expect("IPC request arrives")
            .expect("accept IPC request");
        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        timeout(DEADLINE, reader.read_line(&mut line))
            .await
            .expect("IPC request is complete")
            .expect("read IPC request");
        let request: Value = serde_json::from_str(&line).unwrap();
        assert_eq!(request["operation"], operation);
        assert_eq!(request["payload"]["locationId"], location);
        reader
    }

    async fn status(&mut self, id: u64) {
        self.send(json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": "construct_get_index_status", "arguments": { "locationId": "allowed-a" } }
        }))
        .await;
    }

    async fn eof(&mut self) {
        drop(self.input.take());
        assert!(timeout(DEADLINE, self.child.wait())
            .await
            .expect("EOF terminates the process even while IPC is pending")
            .expect("wait for MCP exit")
            .success());
        let mut stderr = String::new();
        self.child
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut stderr)
            .await
            .expect("read stderr");
        assert!(stderr.is_empty(), "unexpected MCP stderr: {stderr}");
    }
}

impl Drop for Adapter {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
        let _ = fs::remove_dir_all(&self.root);
    }
}

async fn unavailable(stream: &mut BufReader<UnixStream>) {
    stream
        .get_mut()
        .write_all(
            b"{\"protocolVersion\":1,\"result\":null,\"error\":\"Synthetic index unavailable\"}\n",
        )
        .await
        .expect("reply with an unavailable index");
}

async fn disconnected(stream: &mut BufReader<UnixStream>) {
    let mut remaining = Vec::new();
    timeout(DEADLINE, stream.read_to_end(&mut remaining))
        .await
        .expect("adapter releases its pending IPC connection")
        .expect("observe IPC close");
    assert!(remaining.is_empty());
}

#[tokio::test]
async fn direct_and_symlink_startup_serve_protocol_while_index_and_tool_are_pending() {
    for symlink in [false, true] {
        let mut adapter = Adapter::start(symlink);
        let mut initial = adapter.ipc("sync", "allowed-a").await;
        // The initial sync never responds. Discovery must still work.
        adapter.initialize().await;
        adapter
            .send(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" }))
            .await;
        let tools = adapter.response(2).await;
        let tool_names = tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(tool_names, EXPECTED_TOOL_NAMES);

        adapter.status(3).await;
        let mut first = adapter.ipc("status", "allowed-a").await;
        adapter.status(4).await;
        adapter
            .send(json!({ "jsonrpc": "2.0", "id": 5, "method": "ping" }))
            .await;
        assert_eq!(adapter.response(5).await["result"], json!({}));
        adapter
            .send(json!({ "jsonrpc": "2.0", "id": 6, "method": "tools/list" }))
            .await;
        assert_eq!(adapter.response(6).await["result"], tools["result"]);
        assert!(
            timeout(Duration::from_millis(50), adapter.listener.accept())
                .await
                .is_err(),
            "indexed tool calls stay serialized"
        );

        unavailable(&mut first).await;
        assert_eq!(adapter.response(3).await["result"]["isError"], true);
        let mut second = adapter.ipc("status", "allowed-a").await;
        adapter.eof().await;
        disconnected(&mut initial).await;
        disconnected(&mut second).await;
        let log = fs::read_to_string(adapter.root.join("logs/construct.log")).unwrap();
        assert!(log.contains("mcp_adapter_ready"));
        assert!(log.contains("mcp_initial_reconciliation_cancelled"));
        assert!(log.contains("mcp_adapter_stopped"));
        assert!(!log.contains("allowed-a"));
        assert!(!log.contains(adapter.root.to_str().unwrap()));
    }
}

#[tokio::test]
async fn eof_before_initialize_cancels_initial_reconciliation() {
    let mut adapter = Adapter::start(false);
    let mut initial = adapter.ipc("sync", "allowed-a").await;
    adapter.eof().await;
    disconnected(&mut initial).await;
}

#[tokio::test]
async fn failed_initial_reconciliation_continues_only_through_the_allowlist() {
    let mut adapter = Adapter::start(false);
    let mut first = adapter.ipc("sync", "allowed-a").await;
    adapter.initialize().await;
    unavailable(&mut first).await;
    let mut second = adapter.ipc("sync", "allowed-b").await;
    unavailable(&mut second).await;
    timeout(DEADLINE, async {
        loop {
            let log = fs::read_to_string(adapter.root.join("logs/construct.log")).unwrap();
            if log.contains("mcp_initial_reconciliation_completed") {
                assert!(log.contains("\"failureCount\":2"));
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("initial reconciliation is best-effort");
    assert!(
        timeout(Duration::from_millis(100), adapter.listener.accept())
            .await
            .is_err(),
        "the excluded Location must not be reconciled"
    );
    adapter.eof().await;
}

#[tokio::test]
async fn queued_tool_calls_are_bounded_without_blocking_control_or_eof() {
    let mut adapter = Adapter::start(false);
    let mut initial = adapter.ipc("sync", "allowed-a").await;
    adapter.initialize().await;
    adapter.status(100).await;
    let mut active = adapter.ipc("status", "allowed-a").await;
    let first_queued_id = 101;
    let rejected_id = first_queued_id + EXPECTED_MAX_QUEUED_TOOL_CALLS;
    for id in first_queued_id..=rejected_id {
        adapter.status(id).await;
    }
    let rejected = adapter.response(rejected_id).await;
    assert_eq!(rejected["result"]["isError"], true);
    assert_eq!(
        rejected["result"]["structuredContent"]["error"]["code"],
        "server_busy"
    );
    adapter
        .send(json!({ "jsonrpc": "2.0", "id": 200, "method": "ping" }))
        .await;
    assert_eq!(adapter.response(200).await["result"], json!({}));
    adapter.eof().await;
    disconnected(&mut initial).await;
    disconnected(&mut active).await;
}
