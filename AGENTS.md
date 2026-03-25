# Project Context for Codex

You are acting as an independent reviewer for this project.
Your reviews are consumed by Claude Code's AI Dev Team pipeline.

## Output Format
- Be concise — your output is parsed by another AI, not a human
- Use numbered lists for issues/suggestions
- Prefix severity: [critical], [major], [minor], [suggestion]
- Focus on gaps, edge cases, and things the primary agent might miss

## Product
- **Name:** tileport
- **Purpose:** A Rust-based tiling window manager for macOS that brings Linux WM features (i3/Hyprland) to macOS without requiring SIP disable
- **Users:** General macOS users who want tiling window management — power users, developers, Linux WM enthusiasts on Mac
- **Domain:** Desktop productivity / window management

## Stack
- Rust workspace (4 crates: tileport-core, tileport-macos, tileport-cli, tileport-wm)
- macOS platform: objc2, core-graphics, accessibility-sys
- IPC: crossbeam-channel (inter-thread), tokio Unix socket (CLI↔daemon)
- Config: TOML, serde, clap 4
- Logging: tracing + tracing-subscriber

## Conventions
- `tileport-core` is pure Rust — zero platform dependencies, fully testable on any OS
- `tileport-macos` isolates all macOS FFI/platform code
- Internal thread communication via `crossbeam-channel` (lock-free, hot path)
- tokio only for IPC Unix socket server (cold path)
- Test alongside implementation using inline `#[cfg(test)]` modules
- Zero SIP dependency — only public macOS APIs
