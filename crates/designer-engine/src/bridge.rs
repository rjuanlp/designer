//! Revision-checked requests shared by the GUI bridge and MCP server.

use std::path::Path;

use designer_core::{Document, Operation, PageId};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{Session, import::import_figma_file};

/// Windows named pipe used to attach MCP to the running Designer session.
pub const WINDOWS_SESSION_PIPE: &str = r"\\.\pipe\designer-session-v1";

/// A request to read or mutate one open document session.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum SessionRequest {
    /// Read the document and its revision/history metadata.
    ReadDocument,
    /// Describe the model operations and unsupported features.
    DescribeCapabilities,
    /// Apply a single atomic transaction.
    ApplyTransaction {
        /// Revision returned by the latest read.
        expected_revision: u64,
        /// Operations to commit as one undo step.
        operations: Vec<Operation>,
    },
    /// Replace the current document with a new empty document.
    NewDocument {
        /// Revision returned by the latest read.
        expected_revision: u64,
        /// Name for the empty document.
        name: String,
    },
    /// Undo the most recent transaction.
    Undo {
        /// Revision returned by the latest read.
        expected_revision: u64,
    },
    /// Redo the most recently undone transaction.
    Redo {
        /// Revision returned by the latest read.
        expected_revision: u64,
    },
    /// Save to the current document path.
    Save {
        /// Revision returned by the latest read.
        expected_revision: u64,
    },
    /// Save to a path and associate it with the document.
    SaveAs {
        /// Revision returned by the latest read.
        expected_revision: u64,
        /// Destination `.dsg` path.
        path: String,
    },
    /// Import structured Figma JSON and replace the document.
    ImportFigmaJson {
        /// Revision returned by the latest read.
        expected_revision: u64,
        /// Source JSON file.
        path: String,
    },
    /// Evaluate a page at the requested time.
    RenderSnapshot {
        /// Page identifier.
        page_id: u64,
        /// Evaluation time in milliseconds.
        time_ms: u32,
    },
}

/// JSON response carried by the local GUI/MCP bridge.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SessionReply {
    /// The request completed successfully.
    Ok {
        /// Tool result payload.
        result: Value,
    },
    /// The request failed without changing the session.
    Error {
        /// Failure detail suitable for a local MCP tool response.
        error: String,
    },
}

/// Execute a session request and return the JSON payload exposed by MCP.
pub fn execute_session_request(
    session: &mut Session,
    request: SessionRequest,
) -> Result<Value, String> {
    match request {
        SessionRequest::DescribeCapabilities => Ok(json!({
            "schema_version": crate::dsg::SCHEMA_VERSION,
            "operations": Operation::DISCRIMINATORS,
            "prototype_triggers": ["hover", "hover_leave", "click"],
            "unsupported": [
                "components", "styles", "assets", "auto-layout",
                "text editing", "GPU image rendering", "binary .fig"
            ]
        })),
        SessionRequest::ReadDocument => {
            let document = serde_json::to_value(session.document())
                .map_err(|error| format!("serialize document: {error}"))?;
            Ok(json!({
                "revision": session.revision(),
                "dirty": session.is_dirty(),
                "path": session.path().map(|path| path.to_string_lossy().into_owned()),
                "can_undo": session.can_undo(),
                "can_redo": session.can_redo(),
                "document": document
            }))
        }
        SessionRequest::ApplyTransaction {
            expected_revision,
            operations,
        } => {
            check_revision(session, expected_revision)?;
            if operations.is_empty() {
                return Err("operations must contain at least one operation".into());
            }
            let affected = session
                .apply(&operations)
                .map_err(|error| error.to_string())?;
            Ok(json!({
                "revision": session.revision(),
                "affected_ids": affected.iter().map(|id| id.0).collect::<Vec<_>>()
            }))
        }
        SessionRequest::NewDocument {
            expected_revision,
            name,
        } => {
            check_revision(session, expected_revision)?;
            session
                .replace_document(Document::empty(&name))
                .map_err(|error| error.to_string())?;
            Ok(json!({"revision": session.revision()}))
        }
        SessionRequest::Undo { expected_revision } => {
            check_revision(session, expected_revision)?;
            let changed = session.undo();
            Ok(json!({"changed": changed, "revision": session.revision()}))
        }
        SessionRequest::Redo { expected_revision } => {
            check_revision(session, expected_revision)?;
            let changed = session.redo();
            Ok(json!({"changed": changed, "revision": session.revision()}))
        }
        SessionRequest::Save { expected_revision } => {
            check_revision(session, expected_revision)?;
            if !session
                .save()
                .map_err(|error| format!("save document: {error}"))?
            {
                return Err("the session has no file path; use save_as first".into());
            }
            Ok(json!({
                "revision": session.revision(),
                "path": session.path().map(|path| path.to_string_lossy().into_owned())
            }))
        }
        SessionRequest::SaveAs {
            expected_revision,
            path,
        } => {
            check_revision(session, expected_revision)?;
            session
                .save_as(Path::new(&path))
                .map_err(|error| format!("save document: {error}"))?;
            Ok(json!({"revision": session.revision(), "path": path}))
        }
        SessionRequest::ImportFigmaJson {
            expected_revision,
            path,
        } => {
            check_revision(session, expected_revision)?;
            let imported = import_figma_file(Path::new(&path))
                .map_err(|error| format!("import Figma JSON: {error}"))?;
            let report = serde_json::to_value(&imported.report)
                .map_err(|error| format!("serialize import report: {error}"))?;
            session
                .replace_document(imported.document)
                .map_err(|error| error.to_string())?;
            Ok(json!({
                "revision": session.revision(),
                "report": report
            }))
        }
        SessionRequest::RenderSnapshot { page_id, time_ms } => {
            let page = session
                .document()
                .pages
                .iter()
                .find(|page| page.id == PageId(page_id))
                .ok_or_else(|| format!("unknown page id {page_id}"))?;
            let page = serde_json::to_value(page.at_time(time_ms))
                .map_err(|error| format!("serialize rendered page: {error}"))?;
            Ok(json!({
                "revision": session.revision(),
                "time_ms": time_ms,
                "page": page
            }))
        }
    }
}

fn check_revision(session: &Session, expected: u64) -> Result<(), String> {
    if session.revision() == expected {
        Ok(())
    } else {
        Err(format!(
            "revision conflict: expected {expected}, current revision is {}",
            session.revision()
        ))
    }
}

#[cfg(test)]
mod tests {
    use designer_core::{NodeKind, Operation, PageId, Rect};

    use super::*;

    #[test]
    fn requests_share_the_session_revision_and_history() {
        let mut session = Session::new(Document::empty("bridge"));
        let read = execute_session_request(&mut session, SessionRequest::ReadDocument)
            .expect("read document");
        assert_eq!(read["revision"], 0);

        let result = execute_session_request(
            &mut session,
            SessionRequest::ApplyTransaction {
                expected_revision: 0,
                operations: vec![Operation::AddNode {
                    page: PageId(1),
                    parent: None,
                    name: "from MCP".into(),
                    kind: NodeKind::Ellipse,
                    bounds: Rect::new(0.0, 0.0, 20.0, 20.0),
                    fill: None,
                }],
            },
        )
        .expect("apply transaction");
        assert_eq!(result["revision"], 1);
        assert!(session.can_undo());

        let error = execute_session_request(
            &mut session,
            SessionRequest::Undo {
                expected_revision: 0,
            },
        )
        .expect_err("stale revision must fail");
        assert!(error.contains("revision conflict"));
        assert_eq!(session.revision(), 1);
    }
}
