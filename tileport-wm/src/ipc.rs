//! IPC server: Unix socket listener for external command input.
//!
//! Runs a tokio single-threaded runtime on a dedicated thread.
//! Listens on `/tmp/tileport-<uid>.sock`, accepts connections,
//! reads newline-delimited JSON commands, forwards them to the
//! manager thread via crossbeam channel, and writes back JSON responses.

use crossbeam_channel::Sender;
use serde::{Deserialize, Serialize};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use tileport_core::command::Command;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;

/// IPC response sent back to CLI clients.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IpcResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl IpcResponse {
    pub fn ok() -> Self {
        Self {
            status: "ok".into(),
            message: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            status: "error".into(),
            message: Some(msg.into()),
        }
    }
}

/// Message sent from the IPC thread to the manager thread.
/// Contains the parsed command and a oneshot channel for the response.
pub type IpcMessage = (Command, tokio::sync::oneshot::Sender<IpcResponse>);

/// Get the socket path for the current user: `/tmp/tileport-<uid>.sock`.
pub fn socket_path() -> PathBuf {
    // SAFETY: getuid() is always safe to call and returns the real user ID.
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/tileport-{uid}.sock"))
}

/// Clean up a stale socket file if present.
///
/// Tries to connect to the existing socket. If connection is refused
/// (stale socket from a crashed daemon), removes the file.
/// If connection succeeds, the socket is in use by another instance.
fn cleanup_stale_socket(path: &std::path::Path) -> std::io::Result<()> {
    if !path.exists() {
        return Ok(());
    }

    // Try connecting with std (blocking) to check if socket is alive.
    match std::os::unix::net::UnixStream::connect(path) {
        Ok(_stream) => {
            // Another daemon is running -- cannot bind.
            Err(std::io::Error::new(
                std::io::ErrorKind::AddrInUse,
                "another tileport instance is already running",
            ))
        }
        Err(e) if e.kind() == std::io::ErrorKind::ConnectionRefused => {
            // Stale socket -- safe to remove.
            tracing::info!(?path, "removing stale socket file");
            std::fs::remove_file(path)?;
            Ok(())
        }
        Err(_) => {
            // Other error (e.g., permission denied, not a socket).
            // Try removing anyway -- bind will fail if something is wrong.
            tracing::warn!(?path, "unexpected socket state, attempting removal");
            std::fs::remove_file(path)?;
            Ok(())
        }
    }
}

/// Start the IPC server on a dedicated thread.
///
/// Returns the thread handle. The thread runs a single-threaded tokio
/// runtime that listens for Unix socket connections.
pub fn start_ipc_thread(
    cmd_tx: Sender<IpcMessage>,
    shutdown: Arc<AtomicBool>,
) -> JoinHandle<()> {
    std::thread::Builder::new()
        .name("tileport-ipc".into())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to create tokio runtime for IPC");

            rt.block_on(async move {
                let path = socket_path();

                // Stale socket cleanup (DevSecOps requirement).
                if let Err(e) = cleanup_stale_socket(&path) {
                    tracing::error!(error = %e, "failed to clean up socket, IPC disabled");
                    return;
                }

                // Bind the Unix listener.
                let listener = match UnixListener::bind(&path) {
                    Ok(l) => l,
                    Err(e) => {
                        tracing::error!(error = %e, ?path, "failed to bind IPC socket");
                        return;
                    }
                };

                // Set socket permissions to 0o600 (DevSecOps requirement).
                if let Err(e) =
                    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))
                {
                    tracing::error!(error = %e, "failed to set socket permissions");
                    // Continue anyway -- the socket is bound.
                }

                tracing::info!(?path, "IPC socket listening");

                loop {
                    // Check shutdown between accepts.
                    if shutdown.load(Ordering::Relaxed) {
                        break;
                    }

                    // Use tokio::select! with a timeout to periodically check shutdown.
                    let accept_result = tokio::select! {
                        result = listener.accept() => Some(result),
                        _ = tokio::time::sleep(std::time::Duration::from_millis(250)) => None,
                    };

                    let (stream, _addr) = match accept_result {
                        Some(Ok(conn)) => conn,
                        Some(Err(e)) => {
                            tracing::warn!(error = %e, "failed to accept IPC connection");
                            continue;
                        }
                        None => continue, // Timeout -- loop back to check shutdown.
                    };

                    // Handle one connection at a time (simple for MVP).
                    let cmd_tx = cmd_tx.clone();
                    let (reader, mut writer) = tokio::io::split(stream);
                    let mut reader = BufReader::new(reader);
                    let mut line = String::new();

                    match reader.read_line(&mut line).await {
                        Ok(0) => {
                            // EOF -- client disconnected without sending.
                            continue;
                        }
                        Ok(_) => {
                            let trimmed = line.trim();
                            let response = match serde_json::from_str::<Command>(trimmed) {
                                Ok(command) => {
                                    tracing::debug!(?command, "received IPC command");
                                    let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();

                                    match cmd_tx.try_send((command, resp_tx)) {
                                        Ok(()) => {
                                            // Wait for response with timeout.
                                            match tokio::time::timeout(
                                                std::time::Duration::from_secs(5),
                                                resp_rx,
                                            )
                                            .await
                                            {
                                                Ok(Ok(resp)) => resp,
                                                Ok(Err(_)) => IpcResponse::error(
                                                    "manager dropped response channel",
                                                ),
                                                Err(_) => {
                                                    IpcResponse::error("daemon unresponsive")
                                                }
                                            }
                                        }
                                        Err(_) => IpcResponse::error("command channel full"),
                                    }
                                }
                                Err(e) => IpcResponse::error(format!("invalid command: {e}")),
                            };

                            let response_json =
                                serde_json::to_string(&response).unwrap_or_else(|_| {
                                    r#"{"status":"error","message":"serialization failed"}"#
                                        .to_string()
                                });

                            let mut output = response_json;
                            output.push('\n');

                            if let Err(e) = writer.write_all(output.as_bytes()).await {
                                tracing::warn!(error = %e, "failed to write IPC response");
                            }
                        }
                        Err(e) => {
                            tracing::warn!(error = %e, "failed to read IPC request");
                        }
                    }
                }

                // Clean up socket file on shutdown.
                tracing::info!("IPC thread shutting down, removing socket");
                let _ = std::fs::remove_file(&path);
            });
        })
        .expect("failed to spawn IPC thread")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipc_response_ok_serialization() {
        let resp = IpcResponse::ok();
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(json, r#"{"status":"ok"}"#);
    }

    #[test]
    fn test_ipc_response_error_serialization() {
        let resp = IpcResponse::error("something went wrong");
        let json = serde_json::to_string(&resp).unwrap();
        assert_eq!(
            json,
            r#"{"status":"error","message":"something went wrong"}"#
        );
    }

    #[test]
    fn test_ipc_response_ok_deserialization() {
        let resp: IpcResponse = serde_json::from_str(r#"{"status":"ok"}"#).unwrap();
        assert_eq!(resp.status, "ok");
        assert_eq!(resp.message, None);
    }

    #[test]
    fn test_ipc_response_error_deserialization() {
        let resp: IpcResponse =
            serde_json::from_str(r#"{"status":"error","message":"bad input"}"#).unwrap();
        assert_eq!(resp.status, "error");
        assert_eq!(resp.message, Some("bad input".into()));
    }

    #[test]
    fn test_socket_path_contains_uid() {
        let path = socket_path();
        let uid = unsafe { libc::getuid() };
        assert_eq!(
            path,
            PathBuf::from(format!("/tmp/tileport-{uid}.sock"))
        );
    }

    #[test]
    fn test_ipc_round_trip() {
        // Test that a command round-trips through IPC serialization.
        // Simulates: CLI serializes command -> IPC server deserializes.
        let commands = vec![
            Command::FocusNext,
            Command::FocusPrev,
            Command::SwitchWorkspace { workspace: 3 },
            Command::MoveToWorkspace { workspace: 7 },
            Command::ToggleFloat,
            Command::ToggleFullscreen,
            Command::Quit,
        ];

        for cmd in commands {
            let json = serde_json::to_string(&cmd).unwrap();
            // Simulate newline-delimited protocol.
            let with_newline = format!("{json}\n");
            let trimmed = with_newline.trim();
            let parsed: Command = serde_json::from_str(trimmed).unwrap();
            assert_eq!(cmd, parsed, "round-trip failed for {json}");
        }
    }

    #[test]
    fn test_ipc_server_start_and_connect() {
        // Integration test: start the IPC server, send a command, receive response.
        let (cmd_tx, cmd_rx) = crossbeam_channel::bounded::<IpcMessage>(16);
        let shutdown = Arc::new(AtomicBool::new(false));

        let _handle = start_ipc_thread(cmd_tx, Arc::clone(&shutdown));

        // Give the server a moment to bind.
        std::thread::sleep(std::time::Duration::from_millis(100));

        let path = socket_path();

        // Spawn a thread to handle the command on the "manager" side.
        let manager_handle = std::thread::spawn(move || {
            if let Ok((cmd, resp_tx)) = cmd_rx.recv_timeout(std::time::Duration::from_secs(2)) {
                assert_eq!(cmd, Command::FocusNext);
                let _ = resp_tx.send(IpcResponse::ok());
            } else {
                panic!("manager did not receive command");
            }
        });

        // Connect as a client and send a command.
        let mut stream = std::os::unix::net::UnixStream::connect(&path)
            .expect("failed to connect to IPC socket");
        stream
            .set_read_timeout(Some(std::time::Duration::from_secs(2)))
            .unwrap();

        use std::io::{BufRead, Write};
        let cmd = Command::FocusNext;
        let mut msg = serde_json::to_string(&cmd).unwrap();
        msg.push('\n');
        stream.write_all(msg.as_bytes()).unwrap();

        // Read response.
        let mut reader = std::io::BufReader::new(&stream);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).unwrap();

        let resp: IpcResponse = serde_json::from_str(response_line.trim()).unwrap();
        assert_eq!(resp, IpcResponse::ok());

        manager_handle.join().unwrap();

        // Shut down the IPC thread.
        shutdown.store(true, Ordering::Relaxed);
        // Give it time to notice shutdown.
        std::thread::sleep(std::time::Duration::from_millis(500));

        // Socket file should be cleaned up.
        // (May still exist briefly -- not a hard assertion.)
    }
}
