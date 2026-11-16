//! End-to-end JSON-RPC contract for the configured stdio transport.

use std::{process::Stdio, time::Duration};

use designer_core::{NodeKind, Operation, PageId, Rect, sample_document};
use serde_json::{Value, json};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::Command,
    time::timeout,
};

async fn send_json(input: &mut tokio::process::ChildStdin, value: &Value) {
    input
        .write_all(serde_json::to_string(value).expect("JSON").as_bytes())
        .await
        .expect("write JSON-RPC message");
    input.write_all(b"\n").await.expect("write delimiter");
    input.flush().await.expect("flush JSON-RPC message");
}

async fn receive_response(
    output: &mut BufReader<tokio::process::ChildStdout>,
    expected_id: u64,
) -> Value {
    loop {
        let mut line = String::new();
        let bytes = timeout(Duration::from_secs(10), output.read_line(&mut line))
            .await
            .expect("JSON-RPC response timeout")
            .expect("read JSON-RPC response");
        assert_ne!(bytes, 0, "server exited before responding to request");
        let response: Value = serde_json::from_str(&line).expect("JSON-RPC response");
        if response["id"] == json!(expected_id) {
            return response;
        }
    }
}

async fn initialize(
    input: &mut tokio::process::ChildStdin,
    output: &mut BufReader<tokio::process::ChildStdout>,
    request_id: u64,
) {
    send_json(
        input,
        &json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "contract-test", "version": "0.1"}
            }
        }),
    )
    .await;
    send_json(
        input,
        &json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    )
    .await;
    assert!(
        receive_response(output, request_id)
            .await
            .get("result")
            .is_some(),
        "server initialization succeeds"
    );
}

#[tokio::test]
async fn stdio_initializes_lists_tools_and_reads_a_document() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_designer-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("MCP server starts");
    let mut input = child.stdin.take().expect("stdin");
    let mut output = BufReader::new(child.stdout.take().expect("stdout"));
    let requests = [
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-11-25",
                "capabilities": {},
                "clientInfo": {"name": "contract-test", "version": "0.1"}
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {"name": "read_document", "arguments": {}}
        }),
    ];
    let add_node = serde_json::to_value(Operation::AddNode {
        page: PageId(1),
        parent: None,
        name: "mcp shape".to_owned(),
        kind: NodeKind::Ellipse,
        bounds: Rect::new(0.0, 0.0, 25.0, 25.0),
        fill: None,
    })
    .expect("operation JSON");
    for request in requests {
        input
            .write_all(serde_json::to_string(&request).expect("JSON").as_bytes())
            .await
            .expect("write request");
        input.write_all(b"\n").await.expect("write delimiter");
    }
    input.flush().await.expect("flush requests");

    let mut responses = Vec::new();
    while ![1, 2, 3].iter().all(|id| {
        responses
            .iter()
            .any(|response: &Value| response["id"] == json!(id))
    }) {
        let mut line = String::new();
        let bytes = timeout(Duration::from_secs(10), output.read_line(&mut line))
            .await
            .expect("server response timeout")
            .expect("read response");
        if bytes == 0 {
            panic!("server closed before responding to setup calls");
        }
        responses.push(serde_json::from_str::<Value>(&line).expect("JSON-RPC response"));
    }
    let fresh_transaction = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "apply_transaction",
            "arguments": {
                "expected_revision": 0,
                "operations": [add_node]
            }
        }
    });
    input
        .write_all(
            serde_json::to_string(&fresh_transaction)
                .expect("JSON")
                .as_bytes(),
        )
        .await
        .expect("write transaction");
    input.write_all(b"\n").await.expect("write delimiter");
    input.flush().await.expect("flush transaction");
    loop {
        let mut line = String::new();
        let bytes = timeout(Duration::from_secs(10), output.read_line(&mut line))
            .await
            .expect("transaction response timeout")
            .expect("read transaction response");
        if bytes == 0 {
            panic!("server closed before the transaction response");
        }
        responses.push(serde_json::from_str::<Value>(&line).expect("JSON-RPC response"));
        if responses.iter().any(|response| response["id"] == json!(4)) {
            break;
        }
    }
    let stale_transaction = json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "apply_transaction",
            "arguments": {
                "expected_revision": 0,
                "operations": [serde_json::to_value(Operation::AddNode {
                    page: PageId(1),
                    parent: None,
                    name: "stale".to_owned(),
                    kind: NodeKind::Ellipse,
                    bounds: Rect::new(0.0, 0.0, 10.0, 10.0),
                    fill: None,
                })
                .expect("operation JSON")]
            }
        }
    });
    input
        .write_all(
            serde_json::to_string(&stale_transaction)
                .expect("JSON")
                .as_bytes(),
        )
        .await
        .expect("write stale transaction");
    input.write_all(b"\n").await.expect("write delimiter");
    input.flush().await.expect("flush stale transaction");
    loop {
        let mut line = String::new();
        let bytes = timeout(Duration::from_secs(10), output.read_line(&mut line))
            .await
            .expect("stale revision response timeout")
            .expect("read stale response");
        if bytes == 0 {
            panic!("server closed before the stale revision response");
        }
        responses.push(serde_json::from_str::<Value>(&line).expect("JSON-RPC response"));
        if responses.iter().any(|response| response["id"] == json!(5)) {
            break;
        }
    }
    drop(input);
    assert!(responses.iter().any(|response| response["id"] == json!(1)));
    let listed = responses
        .iter()
        .find(|response| response["id"] == json!(2))
        .expect("tools/list response");
    assert!(
        listed["result"]["tools"]
            .as_array()
            .expect("tools")
            .iter()
            .any(|tool| tool["name"] == "read_document")
    );
    let called = responses
        .iter()
        .find(|response| response["id"] == json!(3))
        .unwrap_or_else(|| panic!("tools/call response missing; got {responses:#?}"));
    assert_eq!(
        called["result"]["content"][0]["type"], "text",
        "read_document returns a valid MCP tool result"
    );
    assert!(
        responses
            .iter()
            .find(|response| response["id"] == json!(4))
            .is_some_and(|response| response["result"]["isError"] != true),
        "fresh revision permits a transaction"
    );
    assert!(
        responses
            .iter()
            .find(|response| response["id"] == json!(5))
            .is_some_and(|response| response["error"]["code"] == -32602),
        "stale revision returns an MCP tool error"
    );
    let status = timeout(Duration::from_secs(10), child.wait())
        .await
        .expect("server process exits after stdin closes")
        .expect("server process status");
    assert!(status.success());
}

#[tokio::test]
async fn create_animate_save_and_reopen_round_trips_the_document() {
    let directory =
        std::env::temp_dir().join(format!("designer-mcp-roundtrip-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("temporary directory");
    let path = directory.join("workflow.dsg");
    let mut child = Command::new(env!("CARGO_BIN_EXE_designer-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("MCP server starts");
    let mut input = child.stdin.take().expect("stdin");
    let mut output = BufReader::new(child.stdout.take().expect("stdout"));
    initialize(&mut input, &mut output, 1).await;

    let expected = sample_document();
    let page = &expected.pages[0];
    let operations = vec![
        Operation::AddNode {
            page: page.id,
            parent: None,
            name: "Round-trip shape".to_owned(),
            kind: NodeKind::Ellipse,
            bounds: Rect::new(4.0, 5.0, 26.0, 28.0),
            fill: None,
        },
        Operation::SetKeyframe {
            page: page.id,
            node: page.nodes[0].id,
            property: designer_core::Property::Opacity,
            time_ms: 0,
            value: 0.6,
            easing: designer_core::Easing::Linear,
        },
    ];
    let (expected, _) = expected.transact(&operations).expect("valid workflow");
    send_json(
        &mut input,
        &json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "apply_transaction",
                "arguments": {
                    "expected_revision": 0,
                    "operations": operations
                }
            }
        }),
    )
    .await;
    let transaction = receive_response(&mut output, 2).await;
    assert_ne!(transaction["result"]["isError"], true);

    send_json(
        &mut input,
        &json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "save_as",
                "arguments": {
                    "expected_revision": 1,
                    "path": path.to_string_lossy()
                }
            }
        }),
    )
    .await;
    let saved = receive_response(&mut output, 3).await;
    assert_ne!(saved["result"]["isError"], true);
    drop(input);
    assert!(
        timeout(Duration::from_secs(10), child.wait())
            .await
            .expect("server exit timeout")
            .expect("server status")
            .success()
    );

    let mut child = Command::new(env!("CARGO_BIN_EXE_designer-mcp"))
        .arg("--open")
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("reopened MCP server starts");
    let mut input = child.stdin.take().expect("reopened stdin");
    let mut output = BufReader::new(child.stdout.take().expect("reopened stdout"));
    initialize(&mut input, &mut output, 4).await;
    send_json(
        &mut input,
        &json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {"name": "read_document", "arguments": {}}
        }),
    )
    .await;
    let reopened = receive_response(&mut output, 5).await;
    assert_eq!(
        reopened["result"]["structuredContent"]["document"],
        serde_json::to_value(expected).expect("expected document JSON")
    );
    drop(input);
    assert!(
        timeout(Duration::from_secs(10), child.wait())
            .await
            .expect("reopened server exit timeout")
            .expect("reopened server status")
            .success()
    );
    std::fs::remove_file(path).expect("remove test document");
    std::fs::remove_dir(directory).expect("remove temporary directory");
}
