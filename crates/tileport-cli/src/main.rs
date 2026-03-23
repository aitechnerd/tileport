use clap::{Parser, Subcommand};
use std::io::{BufRead, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::time::Duration;
use tileport_core::command::Command;

#[derive(Parser)]
#[command(name = "tileport", about = "CLI client for the tileport window manager")]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Subcommand)]
enum CliCommand {
    /// Control window focus
    Focus {
        #[command(subcommand)]
        direction: FocusDirection,
    },
    /// Switch to a workspace (1-9)
    Workspace {
        /// Workspace number (1-9)
        #[arg(value_parser = clap::value_parser!(u8).range(1..=9))]
        number: u8,
    },
    /// Move focused window to a workspace (1-9)
    MoveToWorkspace {
        /// Target workspace number (1-9)
        #[arg(value_parser = clap::value_parser!(u8).range(1..=9))]
        number: u8,
    },
    /// Toggle float for the focused window
    Float,
    /// Toggle fullscreen for the focused window
    Fullscreen,
    /// Quit the daemon gracefully
    Quit,
}

#[derive(Subcommand)]
enum FocusDirection {
    /// Focus the next window in the monocle carousel
    Next,
    /// Focus the previous window in the monocle carousel
    Prev,
}

/// Convert CLI subcommand to the core Command enum.
fn to_command(cli_cmd: &CliCommand) -> Command {
    match cli_cmd {
        CliCommand::Focus { direction } => match direction {
            FocusDirection::Next => Command::FocusNext,
            FocusDirection::Prev => Command::FocusPrev,
        },
        CliCommand::Workspace { number } => Command::SwitchWorkspace { workspace: *number },
        CliCommand::MoveToWorkspace { number } => Command::MoveToWorkspace { workspace: *number },
        CliCommand::Float => Command::ToggleFloat,
        CliCommand::Fullscreen => Command::ToggleFullscreen,
        CliCommand::Quit => Command::Quit,
    }
}

/// Get the socket path for the current user: `/tmp/tileport-<uid>.sock`.
fn socket_path() -> PathBuf {
    // SAFETY: getuid() is always safe and returns the real user ID.
    let uid = unsafe { libc::getuid() };
    PathBuf::from(format!("/tmp/tileport-{uid}.sock"))
}

/// IPC response from the daemon.
#[derive(Debug, serde::Deserialize)]
struct IpcResponse {
    status: String,
    message: Option<String>,
}

fn main() {
    let cli = Cli::parse();
    let command = to_command(&cli.command);

    let path = socket_path();

    // Connect to the daemon's Unix socket.
    let mut stream = match UnixStream::connect(&path) {
        Ok(s) => s,
        Err(e) => {
            match e.kind() {
                std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound => {
                    eprintln!("tileport daemon is not running");
                }
                _ => {
                    eprintln!("failed to connect to daemon: {e}");
                }
            }
            std::process::exit(1);
        }
    };

    // Set read timeout to detect unresponsive daemon.
    if let Err(e) = stream.set_read_timeout(Some(Duration::from_secs(5))) {
        eprintln!("failed to set socket timeout: {e}");
        std::process::exit(1);
    }

    // Serialize command to JSON and send with newline delimiter.
    let mut msg = match serde_json::to_string(&command) {
        Ok(json) => json,
        Err(e) => {
            eprintln!("failed to serialize command: {e}");
            std::process::exit(1);
        }
    };
    msg.push('\n');

    if let Err(e) = stream.write_all(msg.as_bytes()) {
        eprintln!("failed to send command: {e}");
        std::process::exit(1);
    }

    // Read response.
    let mut reader = std::io::BufReader::new(&stream);
    let mut response_line = String::new();
    match reader.read_line(&mut response_line) {
        Ok(0) => {
            eprintln!("daemon closed connection without responding");
            std::process::exit(1);
        }
        Ok(_) => {
            match serde_json::from_str::<IpcResponse>(response_line.trim()) {
                Ok(resp) => {
                    if resp.status == "ok" {
                        // Success -- exit silently.
                    } else {
                        let msg = resp.message.unwrap_or_else(|| "unknown error".into());
                        eprintln!("error: {msg}");
                        std::process::exit(1);
                    }
                }
                Err(e) => {
                    eprintln!("invalid response from daemon: {e}");
                    std::process::exit(1);
                }
            }
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::WouldBlock
                || e.kind() == std::io::ErrorKind::TimedOut
            {
                eprintln!("daemon unresponsive");
            } else {
                eprintln!("failed to read response: {e}");
            }
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_focus_next_to_command() {
        let cmd = to_command(&CliCommand::Focus {
            direction: FocusDirection::Next,
        });
        assert_eq!(cmd, Command::FocusNext);
    }

    #[test]
    fn test_focus_prev_to_command() {
        let cmd = to_command(&CliCommand::Focus {
            direction: FocusDirection::Prev,
        });
        assert_eq!(cmd, Command::FocusPrev);
    }

    #[test]
    fn test_workspace_to_command() {
        let cmd = to_command(&CliCommand::Workspace { number: 5 });
        assert_eq!(cmd, Command::SwitchWorkspace { workspace: 5 });
    }

    #[test]
    fn test_move_to_workspace_to_command() {
        let cmd = to_command(&CliCommand::MoveToWorkspace { number: 3 });
        assert_eq!(cmd, Command::MoveToWorkspace { workspace: 3 });
    }

    #[test]
    fn test_float_to_command() {
        let cmd = to_command(&CliCommand::Float);
        assert_eq!(cmd, Command::ToggleFloat);
    }

    #[test]
    fn test_fullscreen_to_command() {
        let cmd = to_command(&CliCommand::Fullscreen);
        assert_eq!(cmd, Command::ToggleFullscreen);
    }

    #[test]
    fn test_quit_to_command() {
        let cmd = to_command(&CliCommand::Quit);
        assert_eq!(cmd, Command::Quit);
    }

    #[test]
    fn test_cli_command_serialization_matches_protocol() {
        // Each CLI command should serialize to the same JSON the IPC server expects.
        let cases: Vec<(CliCommand, &str)> = vec![
            (
                CliCommand::Focus {
                    direction: FocusDirection::Next,
                },
                r#"{"command":"focus_next"}"#,
            ),
            (
                CliCommand::Focus {
                    direction: FocusDirection::Prev,
                },
                r#"{"command":"focus_prev"}"#,
            ),
            (
                CliCommand::Workspace { number: 3 },
                r#"{"command":"switch_workspace","workspace":3}"#,
            ),
            (
                CliCommand::MoveToWorkspace { number: 7 },
                r#"{"command":"move_to_workspace","workspace":7}"#,
            ),
            (CliCommand::Float, r#"{"command":"toggle_float"}"#),
            (
                CliCommand::Fullscreen,
                r#"{"command":"toggle_fullscreen"}"#,
            ),
            (CliCommand::Quit, r#"{"command":"quit"}"#),
        ];

        for (cli_cmd, expected_json) in cases {
            let command = to_command(&cli_cmd);
            let json = serde_json::to_string(&command).unwrap();
            assert_eq!(json, expected_json, "serialization mismatch for {json}");
        }
    }

    #[test]
    fn test_socket_path_format() {
        let path = socket_path();
        let uid = unsafe { libc::getuid() };
        assert_eq!(
            path,
            PathBuf::from(format!("/tmp/tileport-{uid}.sock"))
        );
    }
}
