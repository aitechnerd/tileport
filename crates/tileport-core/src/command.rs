/// All commands the manager thread can process.
///
/// Commands arrive from hotkeys, IPC, or AX observer events.
/// Serializable for IPC transport (JSON over Unix socket).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    FocusNext,
    FocusPrev,
    SwitchWorkspace { workspace: u8 },
    MoveToWorkspace { workspace: u8 },
    ToggleFloat,
    ToggleFullscreen,
    Quit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_focus_next_roundtrip() {
        let cmd = Command::FocusNext;
        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, deserialized);
    }

    #[test]
    fn test_focus_prev_roundtrip() {
        let cmd = Command::FocusPrev;
        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, deserialized);
    }

    #[test]
    fn test_switch_workspace_roundtrip() {
        let cmd = Command::SwitchWorkspace { workspace: 5 };
        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, deserialized);
    }

    #[test]
    fn test_move_to_workspace_roundtrip() {
        let cmd = Command::MoveToWorkspace { workspace: 3 };
        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, deserialized);
    }

    #[test]
    fn test_toggle_float_roundtrip() {
        let cmd = Command::ToggleFloat;
        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, deserialized);
    }

    #[test]
    fn test_toggle_fullscreen_roundtrip() {
        let cmd = Command::ToggleFullscreen;
        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, deserialized);
    }

    #[test]
    fn test_quit_roundtrip() {
        let cmd = Command::Quit;
        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, deserialized);
    }

    #[test]
    fn test_all_commands_serialize_to_expected_json() {
        // Verify the JSON shape matches the IPC protocol spec
        let json = serde_json::to_string(&Command::FocusNext).unwrap();
        assert_eq!(json, r#"{"command":"focus_next"}"#);

        let json = serde_json::to_string(&Command::SwitchWorkspace { workspace: 3 }).unwrap();
        assert_eq!(json, r#"{"command":"switch_workspace","workspace":3}"#);

        let json = serde_json::to_string(&Command::MoveToWorkspace { workspace: 2 }).unwrap();
        assert_eq!(json, r#"{"command":"move_to_workspace","workspace":2}"#);

        let json = serde_json::to_string(&Command::Quit).unwrap();
        assert_eq!(json, r#"{"command":"quit"}"#);
    }

    #[test]
    fn test_deserialize_from_json_strings() {
        // Simulate what the IPC client would send
        let cmd: Command = serde_json::from_str(r#"{"command":"focus_next"}"#).unwrap();
        assert_eq!(cmd, Command::FocusNext);

        let cmd: Command =
            serde_json::from_str(r#"{"command":"switch_workspace","workspace":7}"#).unwrap();
        assert_eq!(cmd, Command::SwitchWorkspace { workspace: 7 });
    }

    #[test]
    fn test_deserialize_invalid_command() {
        let result = serde_json::from_str::<Command>(r#"{"command":"invalid_command"}"#);
        assert!(result.is_err());
    }
}
