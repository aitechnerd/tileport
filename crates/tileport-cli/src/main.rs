use clap::{Parser, Subcommand};

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
        number: u8,
    },
    /// Move focused window to a workspace (1-9)
    MoveToWorkspace {
        /// Target workspace number (1-9)
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

fn main() {
    let _cli = Cli::parse();
    // IPC client implementation deferred to Phase 5.
    eprintln!("tileport CLI: not yet connected to daemon (Phase 5)");
}
