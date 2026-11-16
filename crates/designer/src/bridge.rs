//! Windows named-pipe transport for the running Designer document session.

use std::{io, sync::mpsc, thread, time::Duration};

use designer_engine::bridge::{SessionReply, SessionRequest, WINDOWS_SESSION_PIPE};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    net::windows::named_pipe::{NamedPipeServer, ServerOptions},
    sync::{mpsc as async_mpsc, oneshot},
};

const MAX_REQUEST_BYTES: u64 = 32 * 1024 * 1024;
const MAX_REPLY_BYTES: usize = 32 * 1024 * 1024;

/// One parsed MCP request awaiting execution on GPUI's main thread.
pub struct PendingRequest {
    /// Session operation to execute.
    pub command: SessionRequest,
    /// Receives the result after the GUI session applies the request.
    pub reply: oneshot::Sender<SessionReply>,
}

/// Start the single-instance Windows pipe and fail if its name is already in use.
pub fn start_pipe_server(
    requests: async_mpsc::UnboundedSender<PendingRequest>,
) -> Result<(), String> {
    start_pipe_server_at(requests, WINDOWS_SESSION_PIPE.to_owned())
}

fn start_pipe_server_at(
    requests: async_mpsc::UnboundedSender<PendingRequest>,
    pipe_name: String,
) -> Result<(), String> {
    let (ready_sender, ready_receiver) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("designer-mcp-pipe".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    let _ = ready_sender.send(Err(error.to_string()));
                    return;
                }
            };
            if let Err(error) = runtime.block_on(run_pipe_server(requests, ready_sender, pipe_name))
            {
                eprintln!("Designer MCP pipe stopped: {error}");
            }
        })
        .map_err(|error| format!("start MCP pipe thread: {error}"))?;

    ready_receiver
        .recv_timeout(Duration::from_secs(5))
        .map_err(|error| format!("MCP pipe did not start: {error}"))?
}

async fn run_pipe_server(
    requests: async_mpsc::UnboundedSender<PendingRequest>,
    ready: mpsc::SyncSender<Result<(), String>>,
    pipe_name: String,
) -> io::Result<()> {
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&pipe_name)?;
    let _ = ready.send(Ok(()));
    loop {
        server.connect().await?;
        let next = ServerOptions::new().create(&pipe_name)?;
        let connected = server;
        server = next;
        let requests = requests.clone();
        tokio::spawn(async move {
            if let Err(error) = handle_client(connected, requests).await {
                eprintln!("Designer MCP pipe request failed: {error}");
            }
        });
    }
}

async fn handle_client(
    pipe: NamedPipeServer,
    requests: async_mpsc::UnboundedSender<PendingRequest>,
) -> io::Result<()> {
    let (reader, mut writer) = tokio::io::split(pipe);
    let mut reader = BufReader::new(reader).take(MAX_REQUEST_BYTES + 1);
    let mut line = Vec::new();
    let bytes = reader.read_until(b'\n', &mut line).await?;
    let reply = if bytes == 0 || bytes as u64 > MAX_REQUEST_BYTES {
        SessionReply::Error {
            error: "empty or oversized session request".into(),
        }
    } else {
        match serde_json::from_slice::<SessionRequest>(&line) {
            Ok(command) => {
                let (reply_sender, reply_receiver) = oneshot::channel();
                if requests
                    .send(PendingRequest {
                        command,
                        reply: reply_sender,
                    })
                    .is_err()
                {
                    SessionReply::Error {
                        error: "the Designer session is no longer available".into(),
                    }
                } else {
                    reply_receiver
                        .await
                        .unwrap_or_else(|_| SessionReply::Error {
                            error: "the Designer session closed before replying".into(),
                        })
                }
            }
            Err(error) => SessionReply::Error {
                error: format!("invalid session request: {error}"),
            },
        }
    };

    let response = serde_json::to_vec(&reply)
        .map_err(|error| io::Error::other(format!("serialize MCP pipe response: {error}")))?;
    if response.len() > MAX_REPLY_BYTES {
        return Err(io::Error::other(
            "MCP pipe response exceeded the size limit",
        ));
    }
    writer.write_all(&response).await?;
    writer.write_all(b"\n").await?;
    writer.shutdown().await
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Write};

    use designer_core::Document;
    use designer_engine::{Session, bridge::execute_session_request};
    use serde_json::Value;

    use super::*;

    #[test]
    fn named_pipe_requests_run_against_the_session_receiver() {
        let pipe_name = format!(r"\\.\pipe\designer-session-test-{}", std::process::id());
        let (sender, mut requests) = async_mpsc::unbounded_channel();
        start_pipe_server_at(sender, pipe_name.clone()).expect("server starts");

        let client_pipe = pipe_name.clone();
        let client = thread::spawn(move || {
            let mut pipe = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(client_pipe)
                .expect("connect to named pipe");
            serde_json::to_writer(&mut pipe, &SessionRequest::ReadDocument)
                .expect("serialize request");
            pipe.write_all(b"\n").expect("write delimiter");
            pipe.flush().expect("flush request");
            let mut response = String::new();
            BufReader::new(pipe)
                .read_line(&mut response)
                .expect("read response");
            serde_json::from_str::<Value>(&response).expect("parse response")
        });

        let pending = requests.blocking_recv().expect("GUI request");
        let mut session = Session::new(Document::empty("live"));
        let result = execute_session_request(&mut session, pending.command).expect("execute read");
        pending
            .reply
            .send(SessionReply::Ok { result })
            .expect("reply to pipe client");

        let response = client.join().expect("client thread exits");
        assert_eq!(response["status"], "ok");
        assert_eq!(response["result"]["document"]["name"], "live");
    }
}
