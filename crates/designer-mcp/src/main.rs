//! Stdio MCP server for editing a single local Designer document session.

use std::{
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use designer_core::Operation;
use designer_engine::{
    Session,
    bridge::{SessionReply, SessionRequest, execute_session_request},
};
use rmcp::{
    ErrorData, Json, ServerHandler, ServiceExt,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{ServerCapabilities, ServerConfig},
    schemars, tool, tool_handler, tool_router,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone)]
struct DesignerMcp {
    backend: SessionBackend,
    // RMCP's generated handler dispatch accesses this field outside the visible impl.
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

#[derive(Clone)]
enum SessionBackend {
    Local(Arc<Mutex<Session>>),
    #[cfg(windows)]
    WindowsPipe(PathBuf),
}

#[derive(Deserialize, schemars::JsonSchema)]
struct RevisionInput {
    #[schemars(description = "Current document revision; read it again after a conflict.")]
    expected_revision: u64,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct TransactionInput {
    #[schemars(description = "Current document revision.")]
    expected_revision: u64,
    #[schemars(
        description = "Array of designer_core::Operation objects. Each operation has an `op` discriminator; a transaction is atomic and creates one undo step."
    )]
    operations: Vec<Value>,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct NewDocumentInput {
    expected_revision: u64,
    name: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct SaveAsInput {
    expected_revision: u64,
    path: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct ImportInput {
    expected_revision: u64,
    path: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
struct SnapshotInput {
    page_id: u64,
    time_ms: u32,
}

impl DesignerMcp {
    fn new(session: Session) -> Self {
        Self {
            backend: SessionBackend::Local(Arc::new(Mutex::new(session))),
            tool_router: Self::tool_router(),
        }
    }

    #[cfg(windows)]
    fn connect_windows_pipe(path: PathBuf) -> Self {
        Self {
            backend: SessionBackend::WindowsPipe(path),
            tool_router: Self::tool_router(),
        }
    }

    fn request(&self, request: SessionRequest) -> Result<Json<Value>, ErrorData> {
        let result = match &self.backend {
            SessionBackend::Local(session) => {
                let mut session = session.lock().map_err(|error| {
                    ErrorData::internal_error(
                        format!("document session lock failed: {error}"),
                        None,
                    )
                })?;
                execute_session_request(&mut session, request)
            }
            #[cfg(windows)]
            SessionBackend::WindowsPipe(path) => request_pipe(path, request),
        };
        result
            .map(Json)
            .map_err(|error| ErrorData::invalid_params(error, None))
    }
}

#[cfg(windows)]
fn request_pipe(path: &Path, request: SessionRequest) -> Result<Value, String> {
    const MAX_REPLY_BYTES: u64 = 16 * 1024 * 1024;
    let mut pipe = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|error| format!("cannot connect to the running Designer session: {error}"))?;
    serde_json::to_writer(&mut pipe, &request)
        .map_err(|error| format!("serialize session request: {error}"))?;
    pipe.write_all(b"\n")
        .and_then(|()| pipe.flush())
        .map_err(|error| format!("send session request: {error}"))?;

    let mut line = Vec::new();
    let bytes = BufReader::new(pipe)
        .take(MAX_REPLY_BYTES + 1)
        .read_until(b'\n', &mut line)
        .map_err(|error| format!("read session response: {error}"))?;
    if bytes == 0 || bytes as u64 > MAX_REPLY_BYTES {
        return Err("invalid or oversized session response".into());
    }
    match serde_json::from_slice::<SessionReply>(&line)
        .map_err(|error| format!("invalid session response: {error}"))?
    {
        SessionReply::Ok { result } => Ok(result),
        SessionReply::Error { error } => Err(error),
    }
}

#[tool_router]
impl DesignerMcp {
    #[tool(description = "Describe transport, schema, supported operations and known limitations.")]
    fn describe_capabilities(&self) -> Result<Json<Value>, ErrorData> {
        let Json(mut result) = self.request(SessionRequest::DescribeCapabilities)?;
        result["transport"] = Value::String(
            match &self.backend {
                SessionBackend::Local(_) => "headless",
                #[cfg(windows)]
                SessionBackend::WindowsPipe(_) => "live",
            }
            .into(),
        );
        Ok(Json(result))
    }
    #[tool(
        description = "Read the entire current document, revision, dirty state, and undo/redo availability."
    )]
    fn read_document(&self) -> Result<Json<Value>, ErrorData> {
        self.request(SessionRequest::ReadDocument)
    }

    #[tool(
        description = "Atomically apply typed designer_core::Operation values at the expected revision. A successful transaction is one undo step."
    )]
    fn apply_transaction(
        &self,
        Parameters(input): Parameters<TransactionInput>,
    ) -> Result<Json<Value>, ErrorData> {
        let operations: Vec<Operation> = input
            .operations
            .into_iter()
            .map(serde_json::from_value)
            .collect::<Result<_, _>>()
            .map_err(|error| {
                ErrorData::invalid_params(format!("invalid operation: {error}"), None)
            })?;
        self.request(SessionRequest::ApplyTransaction {
            expected_revision: input.expected_revision,
            operations,
        })
    }

    #[tool(description = "Create an empty document as one undoable change.")]
    fn new_document(
        &self,
        Parameters(input): Parameters<NewDocumentInput>,
    ) -> Result<Json<Value>, ErrorData> {
        self.request(SessionRequest::NewDocument {
            expected_revision: input.expected_revision,
            name: input.name,
        })
    }

    #[tool(description = "Undo the latest document transaction at the expected revision.")]
    fn undo(&self, Parameters(input): Parameters<RevisionInput>) -> Result<Json<Value>, ErrorData> {
        self.request(SessionRequest::Undo {
            expected_revision: input.expected_revision,
        })
    }

    #[tool(description = "Redo the latest undone transaction at the expected revision.")]
    fn redo(&self, Parameters(input): Parameters<RevisionInput>) -> Result<Json<Value>, ErrorData> {
        self.request(SessionRequest::Redo {
            expected_revision: input.expected_revision,
        })
    }

    #[tool(description = "Save to the document's current .dsg path at the expected revision.")]
    fn save(&self, Parameters(input): Parameters<RevisionInput>) -> Result<Json<Value>, ErrorData> {
        self.request(SessionRequest::Save {
            expected_revision: input.expected_revision,
        })
    }

    #[tool(
        description = "Save the current document to a local .dsg file and make that its current path."
    )]
    fn save_as(
        &self,
        Parameters(input): Parameters<SaveAsInput>,
    ) -> Result<Json<Value>, ErrorData> {
        self.request(SessionRequest::SaveAs {
            expected_revision: input.expected_revision,
            path: input.path,
        })
    }

    #[tool(
        description = "Import a structured Figma JSON file, replace the current document as one undoable change, and return the loss report."
    )]
    fn import_figma_json(
        &self,
        Parameters(input): Parameters<ImportInput>,
    ) -> Result<Json<Value>, ErrorData> {
        self.request(SessionRequest::ImportFigmaJson {
            expected_revision: input.expected_revision,
            path: input.path,
        })
    }

    #[tool(
        description = "Evaluate one page's keyframes at an explicit time and return the still document snapshot without requiring a GPU."
    )]
    fn render_snapshot(
        &self,
        Parameters(input): Parameters<SnapshotInput>,
    ) -> Result<Json<Value>, ErrorData> {
        self.request(SessionRequest::RenderSnapshot {
            page_id: input.page_id,
            time_ms: input.time_ms,
        })
    }
}

#[tool_handler]
impl ServerHandler for DesignerMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_instructions(
                "Designer local document server. Read the current revision before every mutation. Transactions are atomic; on a revision conflict, reread instead of retrying blindly. No shell or script execution is exposed.",
            )
    }
}

fn load_session(path: Option<PathBuf>) -> Result<Session, String> {
    match path {
        Some(path) => Session::open(&path).map_err(|error| error.to_string()),
        None => Ok(Session::new(designer_core::sample_document())),
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut open = None;
    let mut connect = false;
    let mut pipe = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--connect" => connect = true,
            "--pipe" => {
                pipe = Some(PathBuf::from(
                    arguments.next().ok_or("--pipe requires a pipe name")?,
                ))
            }
            "--open" => {
                open = Some(PathBuf::from(
                    arguments.next().ok_or("--open requires a .dsg path")?,
                ));
            }
            other => return Err(format!("unknown argument {other}").into()),
        }
    }
    if pipe.is_some() && !connect {
        return Err("--pipe requires --connect".into());
    }
    if connect && open.is_some() {
        return Err("--connect cannot be combined with --open".into());
    }
    let server =
        if connect {
            #[cfg(windows)]
            {
                DesignerMcp::connect_windows_pipe(pipe.unwrap_or_else(|| {
                    PathBuf::from(designer_engine::bridge::WINDOWS_SESSION_PIPE)
                }))
            }
            #[cfg(not(windows))]
            {
                return Err("--connect is currently supported on Windows only".into());
            }
        } else {
            DesignerMcp::new(load_session(open)?)
        };
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use designer_core::{Document, NodeId, NodeKind, PageId, Rect};

    #[test]
    fn tool_registry_advertises_the_supported_session_capabilities() {
        let server = DesignerMcp::new(Session::new(Document::empty("test")));
        let tools = server.tool_router.list_all();
        let names: Vec<String> = tools.iter().map(|tool| tool.name.to_string()).collect();
        for expected in [
            "read_document",
            "apply_transaction",
            "new_document",
            "undo",
            "redo",
            "save",
            "save_as",
            "import_figma_json",
            "render_snapshot",
        ] {
            assert!(
                names.iter().any(|name| name == expected),
                "tool {expected} is not registered"
            );
        }
    }

    #[test]
    fn prototype_states_are_reachable_and_undoable() {
        let server = DesignerMcp::new(Session::new(designer_core::sample_document()));
        let Json(before) = server.read_document().expect("read");
        let page = &before["document"]["pages"][0];
        let node = page["nodes"][0]["id"].clone();
        let prototype = serde_json::json!({
            "initial": 0, "states": [
                {"id":0,"name":"Initial","overrides":[]},
                {"id":1,"name":"Hover","overrides":[{"node":node,"opacity":0.5}]},
                {"id":2,"name":"Click","overrides":[{"node":node,"visible":false}]}
            ],
            "transitions":[
                {"node":node,"from":0,"to":1,"trigger":"hover","duration_ms":200},
                {"node":node,"from":1,"to":2,"trigger":"click","duration_ms":200}
            ]
        });
        server.apply_transaction(Parameters(TransactionInput { expected_revision: 0, operations: vec![
            serde_json::json!({"op":"set_prototype","page":page["id"],"prototype":prototype})
        ]})).expect("prototype transaction");
        let Json(after) = server.read_document().expect("read");
        assert_eq!(
            after["document"]["pages"][0]["prototype"]["states"][1]["overrides"][0]["opacity"],
            0.5
        );
        server
            .undo(Parameters(RevisionInput {
                expected_revision: 1,
            }))
            .expect("undo");
        assert_eq!(
            server.read_document().expect("read").0["document"],
            before["document"]
        );
        assert_eq!(
            server.describe_capabilities().expect("capabilities").0["transport"],
            "headless"
        );
    }

    #[test]
    fn transactions_are_atomic_and_reject_stale_revisions() {
        let server = DesignerMcp::new(Session::new(Document::empty("test")));
        let valid = Operation::AddNode {
            page: PageId(1),
            parent: None,
            name: "shape".to_owned(),
            kind: NodeKind::Ellipse,
            bounds: Rect::new(0.0, 0.0, 20.0, 20.0),
            fill: None,
        };
        let invalid = Operation::SetBounds {
            page: PageId(1),
            node: NodeId(999),
            bounds: Rect::new(1.0, 1.0, 10.0, 10.0),
        };
        assert!(
            server
                .apply_transaction(Parameters(TransactionInput {
                    expected_revision: 0,
                    operations: vec![
                        serde_json::to_value(valid.clone()).expect("operation JSON"),
                        serde_json::to_value(invalid).expect("operation JSON"),
                    ],
                }))
                .is_err()
        );
        {
            let SessionBackend::Local(session) = &server.backend else {
                panic!("test server must use a local session");
            };
            let session = session.lock().expect("lock");
            assert_eq!(session.revision(), 0);
            assert!(session.document().pages[0].nodes.is_empty());
        }

        server
            .apply_transaction(Parameters(TransactionInput {
                expected_revision: 0,
                operations: vec![serde_json::to_value(valid).expect("operation JSON")],
            }))
            .expect("valid transaction");
        assert!(
            server
                .apply_transaction(Parameters(TransactionInput {
                    expected_revision: 0,
                    operations: vec![
                        serde_json::to_value(Operation::AddNode {
                            page: PageId(1),
                            parent: None,
                            name: "stale".to_owned(),
                            kind: NodeKind::Ellipse,
                            bounds: Rect::new(0.0, 0.0, 10.0, 10.0),
                            fill: None,
                        })
                        .expect("operation JSON")
                    ]
                }))
                .is_err(),
            "stale writes must conflict even when their payload is empty"
        );
    }
}
