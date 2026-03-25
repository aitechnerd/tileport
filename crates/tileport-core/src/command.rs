use crate::zone::Direction;

/// All commands the manager thread can process.
///
/// Commands arrive from hotkeys, IPC, or AX observer events.
/// Serializable for IPC transport (JSON over Unix socket).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Command {
    FocusNext,
    FocusPrev,
    FocusDirection { direction: Direction },
    MoveToZone { direction: Direction },
    PromoteToPrimary,
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
    fn test_focus_direction_roundtrip() {
        let cmd = Command::FocusDirection {
            direction: Direction::Left,
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, deserialized);
    }

    #[test]
    fn test_move_to_zone_roundtrip() {
        let cmd = Command::MoveToZone {
            direction: Direction::Right,
        };
        let json = serde_json::to_string(&cmd).unwrap();
        let deserialized: Command = serde_json::from_str(&json).unwrap();
        assert_eq!(cmd, deserialized);
    }

    #[test]
    fn test_promote_to_primary_roundtrip() {
        let cmd = Command::PromoteToPrimary;
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

        let json = serde_json::to_string(&Command::FocusDirection {
            direction: Direction::Left,
        })
        .unwrap();
        assert_eq!(json, r#"{"command":"focus_direction","direction":"left"}"#);

        let json = serde_json::to_string(&Command::MoveToZone {
            direction: Direction::Right,
        })
        .unwrap();
        assert_eq!(json, r#"{"command":"move_to_zone","direction":"right"}"#);

        let json = serde_json::to_string(&Command::PromoteToPrimary).unwrap();
        assert_eq!(json, r#"{"command":"promote_to_primary"}"#);
    }

    #[test]
    fn test_deserialize_from_json_strings() {
        // Simulate what the IPC client would send
        let cmd: Command = serde_json::from_str(r#"{"command":"focus_next"}"#).unwrap();
        assert_eq!(cmd, Command::FocusNext);

        let cmd: Command =
            serde_json::from_str(r#"{"command":"switch_workspace","workspace":7}"#).unwrap();
        assert_eq!(cmd, Command::SwitchWorkspace { workspace: 7 });

        let cmd: Command =
            serde_json::from_str(r#"{"command":"focus_direction","direction":"left"}"#).unwrap();
        assert_eq!(
            cmd,
            Command::FocusDirection {
                direction: Direction::Left,
            }
        );

        let cmd: Command =
            serde_json::from_str(r#"{"command":"move_to_zone","direction":"right"}"#).unwrap();
        assert_eq!(
            cmd,
            Command::MoveToZone {
                direction: Direction::Right,
            }
        );

        let cmd: Command =
            serde_json::from_str(r#"{"command":"promote_to_primary"}"#).unwrap();
        assert_eq!(cmd, Command::PromoteToPrimary);
    }

    #[test]
    fn test_deserialize_invalid_command() {
        let result = serde_json::from_str::<Command>(r#"{"command":"invalid_command"}"#);
        assert!(result.is_err());
    }
}
