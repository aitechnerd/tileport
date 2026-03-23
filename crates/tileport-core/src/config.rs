use crate::command::Command;
use crate::types::Gaps;

/// Keyboard modifier keys.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Modifier {
    Alt,
    Shift,
    Ctrl,
    Cmd,
}

/// A key identifier for keybinding configuration.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(untagged)]
pub enum Key {
    Char(String),
}

/// A single keybinding mapping a key combination to a command.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Keybinding {
    pub modifiers: Vec<Modifier>,
    pub key: Key,
    pub command: Command,
}

/// Application configuration loaded from TOML with hardcoded defaults.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    #[serde(default)]
    pub gaps: Gaps,
    #[serde(default = "default_keybindings")]
    pub keybindings: Vec<Keybinding>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gaps: Gaps::default(),
            keybindings: default_keybindings(),
        }
    }
}

/// Default keybindings matching SOW MS-05.
fn default_keybindings() -> Vec<Keybinding> {
    let mut bindings = vec![
        Keybinding {
            modifiers: vec![Modifier::Alt],
            key: Key::Char("j".into()),
            command: Command::FocusNext,
        },
        Keybinding {
            modifiers: vec![Modifier::Alt],
            key: Key::Char("k".into()),
            command: Command::FocusPrev,
        },
        Keybinding {
            modifiers: vec![Modifier::Alt],
            key: Key::Char("f".into()),
            command: Command::ToggleFullscreen,
        },
        Keybinding {
            modifiers: vec![Modifier::Alt, Modifier::Shift],
            key: Key::Char("f".into()),
            command: Command::ToggleFloat,
        },
    ];

    // alt+1..9 = SwitchWorkspace(1..9)
    for i in 1..=9u8 {
        bindings.push(Keybinding {
            modifiers: vec![Modifier::Alt],
            key: Key::Char(i.to_string()),
            command: Command::SwitchWorkspace { workspace: i },
        });
    }

    // alt+shift+1..9 = MoveToWorkspace(1..9)
    for i in 1..=9u8 {
        bindings.push(Keybinding {
            modifiers: vec![Modifier::Alt, Modifier::Shift],
            key: Key::Char(i.to_string()),
            command: Command::MoveToWorkspace { workspace: i },
        });
    }

    bindings
}

/// Errors that can occur during config loading.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse TOML: {0}")]
    Parse(#[from] toml::de::Error),
}

impl Config {
    /// Load configuration from a TOML string.
    ///
    /// Missing fields fall back to defaults via serde `default` attributes.
    pub fn from_toml(toml_str: &str) -> Result<Self, ConfigError> {
        let config: Config = toml::from_str(toml_str)?;
        Ok(config)
    }

    /// Load configuration from a file path.
    ///
    /// Returns `Ok(Config::default())` if the file does not exist.
    /// Returns an error if the file exists but cannot be parsed.
    pub fn load_from_file(path: &std::path::Path) -> Result<Self, ConfigError> {
        match std::fs::read_to_string(path) {
            Ok(contents) => Self::from_toml(&contents),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::info!("no config file found at {}, using defaults", path.display());
                Ok(Self::default())
            }
            Err(e) => Err(ConfigError::Io(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.gaps.inner, 8.0);
        assert_eq!(config.gaps.outer, 8.0);
        assert!(!config.keybindings.is_empty());
    }

    #[test]
    fn test_default_keybindings_contain_focus_next() {
        let config = Config::default();
        let has_focus_next = config.keybindings.iter().any(|b| {
            b.command == Command::FocusNext
                && b.modifiers == vec![Modifier::Alt]
                && b.key == Key::Char("j".into())
        });
        assert!(has_focus_next, "default keybindings should contain alt+j = FocusNext");
    }

    #[test]
    fn test_default_keybindings_contain_workspace_switch() {
        let config = Config::default();
        for i in 1..=9u8 {
            let has_switch = config.keybindings.iter().any(|b| {
                b.command == Command::SwitchWorkspace { workspace: i }
                    && b.modifiers == vec![Modifier::Alt]
                    && b.key == Key::Char(i.to_string())
            });
            assert!(
                has_switch,
                "default keybindings should contain alt+{i} = SwitchWorkspace({i})"
            );
        }
    }

    #[test]
    fn test_default_keybindings_contain_move_to_workspace() {
        let config = Config::default();
        for i in 1..=9u8 {
            let has_move = config.keybindings.iter().any(|b| {
                b.command == Command::MoveToWorkspace { workspace: i }
                    && b.modifiers == vec![Modifier::Alt, Modifier::Shift]
                    && b.key == Key::Char(i.to_string())
            });
            assert!(
                has_move,
                "default keybindings should contain alt+shift+{i} = MoveToWorkspace({i})"
            );
        }
    }

    #[test]
    fn test_default_keybinding_count() {
        let config = Config::default();
        // 4 (focus_next, focus_prev, fullscreen, float) + 9 (switch) + 9 (move) = 22
        assert_eq!(config.keybindings.len(), 22);
    }

    #[test]
    fn test_parse_full_toml() {
        let toml_str = r#"
[gaps]
inner = 4.0
outer = 12.0

[[keybindings]]
modifiers = ["alt"]
key = "j"
command = { command = "focus_next" }

[[keybindings]]
modifiers = ["alt"]
key = "k"
command = { command = "focus_prev" }
"#;
        let config = Config::from_toml(toml_str).unwrap();
        assert_eq!(config.gaps.inner, 4.0);
        assert_eq!(config.gaps.outer, 12.0);
        assert_eq!(config.keybindings.len(), 2);
    }

    #[test]
    fn test_parse_gaps_only_uses_default_keybindings() {
        let toml_str = r#"
[gaps]
inner = 16.0
outer = 16.0
"#;
        let config = Config::from_toml(toml_str).unwrap();
        assert_eq!(config.gaps.inner, 16.0);
        assert_eq!(config.gaps.outer, 16.0);
        // Missing keybindings field falls back to default
        assert_eq!(config.keybindings.len(), 22);
    }

    #[test]
    fn test_parse_empty_toml_uses_all_defaults() {
        let config = Config::from_toml("").unwrap();
        assert_eq!(config.gaps.inner, 8.0);
        assert_eq!(config.gaps.outer, 8.0);
        assert_eq!(config.keybindings.len(), 22);
    }

    #[test]
    fn test_load_missing_file_uses_defaults() {
        let path = std::path::Path::new("/tmp/tileport-test-nonexistent-config.toml");
        let config = Config::load_from_file(path).unwrap();
        assert_eq!(config.gaps.inner, 8.0);
        assert_eq!(config.gaps.outer, 8.0);
    }

    #[test]
    fn test_parse_invalid_toml_returns_error() {
        let result = Config::from_toml("this is not valid toml {{{");
        assert!(result.is_err());
    }

    #[test]
    fn test_shipped_default_config_parses() {
        let toml_str = include_str!("../../../config/default-config.toml");
        let config = Config::from_toml(toml_str).unwrap();
        assert_eq!(config.gaps.inner, 8.0);
        assert_eq!(config.gaps.outer, 8.0);
        assert_eq!(config.keybindings.len(), 22);
    }
}
