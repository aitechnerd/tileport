use std::collections::HashMap;

use crate::command::Command;
use crate::types::Gaps;
use crate::workspace::WorkspaceLayout;
use crate::zone::{make_fill_order, normalize_ratios, Direction, ZoneLayout, ZoneNode};

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

/// Layout definition parsed from `[layouts.<name>]` in TOML config.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LayoutConfig {
    #[serde(rename = "type")]
    pub layout_type: String,
    #[serde(default)]
    pub ratios: Vec<f64>,
    /// Sub-splits keyed by child index (as string in TOML, e.g. `[layouts.x.splits.1]`).
    #[serde(default)]
    pub splits: HashMap<String, LayoutConfig>,
}

/// Workspace-to-layout assignment parsed from `[workspaces]` in TOML config.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkspaceConfig {
    pub layout: String,
}

/// Application configuration loaded from TOML with hardcoded defaults.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Config {
    #[serde(default)]
    pub gaps: Gaps,
    #[serde(default = "default_keybindings")]
    pub keybindings: Vec<Keybinding>,
    #[serde(default)]
    pub layouts: HashMap<String, LayoutConfig>,
    /// Workspace-to-layout assignments, keyed by workspace number as string (TOML keys are strings).
    #[serde(default)]
    pub workspaces: HashMap<String, WorkspaceConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            gaps: Gaps::default(),
            keybindings: default_keybindings(),
            layouts: HashMap::new(),
            workspaces: HashMap::new(),
        }
    }
}

/// Default keybindings for zone tiling.
///
/// alt+j/k = FocusDirection Down/Up (replaces FocusNext/FocusPrev)
/// alt+h/l = FocusDirection Left/Right
/// alt+shift+h/l = MoveToZone Left/Right
/// alt+enter = PromoteToPrimary
/// alt+f = ToggleFullscreen, alt+shift+f = ToggleFloat
/// alt+1..9 = SwitchWorkspace, alt+shift+1..9 = MoveToWorkspace
fn default_keybindings() -> Vec<Keybinding> {
    let mut bindings = vec![
        // Direction-based focus (replaces FocusNext/FocusPrev in defaults)
        Keybinding {
            modifiers: vec![Modifier::Alt],
            key: Key::Char("j".into()),
            command: Command::FocusDirection {
                direction: Direction::Down,
            },
        },
        Keybinding {
            modifiers: vec![Modifier::Alt],
            key: Key::Char("k".into()),
            command: Command::FocusDirection {
                direction: Direction::Up,
            },
        },
        Keybinding {
            modifiers: vec![Modifier::Alt],
            key: Key::Char("h".into()),
            command: Command::FocusDirection {
                direction: Direction::Left,
            },
        },
        Keybinding {
            modifiers: vec![Modifier::Alt],
            key: Key::Char("l".into()),
            command: Command::FocusDirection {
                direction: Direction::Right,
            },
        },
        // Move to zone
        Keybinding {
            modifiers: vec![Modifier::Alt, Modifier::Shift],
            key: Key::Char("h".into()),
            command: Command::MoveToZone {
                direction: Direction::Left,
            },
        },
        Keybinding {
            modifiers: vec![Modifier::Alt, Modifier::Shift],
            key: Key::Char("l".into()),
            command: Command::MoveToZone {
                direction: Direction::Right,
            },
        },
        // Promote to primary
        Keybinding {
            modifiers: vec![Modifier::Alt],
            key: Key::Char("enter".into()),
            command: Command::PromoteToPrimary,
        },
        // Existing
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

/// Maximum recursion depth for nested splits in config parsing.
/// C5: Prevents stack overflow from pathological configs.
const MAX_SPLITS_DEPTH: usize = 4;

/// Build a `ZoneNode` tree from a `LayoutConfig`, respecting recursion depth limit (C5).
///
/// Returns `None` for monocle layout_type.
/// Returns a `ZoneNode` for columns/rows.
/// Unknown layout_type logs a warning and returns `None` (C4: monocle fallback).
fn build_zone_node(config: &LayoutConfig, depth: usize) -> Option<ZoneNode> {
    if depth > MAX_SPLITS_DEPTH {
        tracing::warn!(
            depth,
            "layout config exceeds maximum nesting depth of {}, treating as leaf",
            MAX_SPLITS_DEPTH
        );
        return Some(ZoneNode::Leaf);
    }

    match config.layout_type.as_str() {
        "columns" | "rows" => {
            let ratios = if config.ratios.is_empty() {
                vec![0.5, 0.5]
            } else {
                normalize_ratios(&config.ratios)
            };
            let children: Vec<ZoneNode> = (0..ratios.len())
                .map(|i| {
                    config
                        .splits
                        .get(&i.to_string())
                        .and_then(|split_config| build_zone_node(split_config, depth + 1))
                        .unwrap_or(ZoneNode::Leaf)
                })
                .collect();
            if config.layout_type == "columns" {
                Some(ZoneNode::HSplit { ratios, children })
            } else {
                Some(ZoneNode::VSplit { ratios, children })
            }
        }
        "monocle" => None,
        unknown => {
            tracing::warn!(
                layout_type = unknown,
                "unknown layout type, falling back to monocle"
            );
            None
        }
    }
}

impl Config {
    /// Build workspace layouts from the parsed config.
    ///
    /// For each workspace assignment, looks up the layout by name and converts
    /// it to a `WorkspaceLayout`. Unknown layout names or types fall back to monocle.
    pub fn build_workspace_layouts(&self) -> HashMap<u8, WorkspaceLayout> {
        let mut result = HashMap::new();

        for (ws_key, ws_config) in &self.workspaces {
            let workspace_id: u8 = match ws_key.parse() {
                Ok(id) if (1..=9).contains(&id) => id,
                _ => {
                    tracing::warn!(
                        workspace = ws_key,
                        "invalid workspace number (must be 1-9), skipping"
                    );
                    continue;
                }
            };
            let layout_name = &ws_config.layout;
            let layout = match self.layouts.get(layout_name) {
                Some(layout_config) => match build_zone_node(layout_config, 0) {
                    Some(root) => {
                        let fill_order = make_fill_order(&root);
                        let primary_zone = fill_order[0];
                        let zone_layout =
                            ZoneLayout::new(root, fill_order, primary_zone);
                        WorkspaceLayout::Zone(zone_layout)
                    }
                    None => {
                        // monocle or unknown type fallback
                        WorkspaceLayout::Monocle(crate::monocle::MonocleLayout::new())
                    }
                },
                None => {
                    tracing::warn!(
                        layout = layout_name,
                        workspace = workspace_id,
                        "workspace references unknown layout, falling back to monocle"
                    );
                    WorkspaceLayout::Monocle(crate::monocle::MonocleLayout::new())
                }
            };
            result.insert(workspace_id, layout);
        }

        result
    }
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
        assert!(config.layouts.is_empty());
        assert!(config.workspaces.is_empty());
    }

    #[test]
    fn test_default_keybindings_contain_focus_direction_down() {
        let config = Config::default();
        let has_focus_down = config.keybindings.iter().any(|b| {
            b.command
                == Command::FocusDirection {
                    direction: Direction::Down,
                }
                && b.modifiers == vec![Modifier::Alt]
                && b.key == Key::Char("j".into())
        });
        assert!(
            has_focus_down,
            "default keybindings should contain alt+j = FocusDirection(Down)"
        );
    }

    #[test]
    fn test_default_keybindings_contain_focus_direction_left_right() {
        let config = Config::default();
        let has_left = config.keybindings.iter().any(|b| {
            b.command
                == Command::FocusDirection {
                    direction: Direction::Left,
                }
                && b.modifiers == vec![Modifier::Alt]
                && b.key == Key::Char("h".into())
        });
        let has_right = config.keybindings.iter().any(|b| {
            b.command
                == Command::FocusDirection {
                    direction: Direction::Right,
                }
                && b.modifiers == vec![Modifier::Alt]
                && b.key == Key::Char("l".into())
        });
        assert!(has_left, "should contain alt+h = FocusDirection(Left)");
        assert!(has_right, "should contain alt+l = FocusDirection(Right)");
    }

    #[test]
    fn test_default_keybindings_contain_move_to_zone() {
        let config = Config::default();
        let has_move_left = config.keybindings.iter().any(|b| {
            b.command
                == Command::MoveToZone {
                    direction: Direction::Left,
                }
                && b.modifiers == vec![Modifier::Alt, Modifier::Shift]
                && b.key == Key::Char("h".into())
        });
        let has_move_right = config.keybindings.iter().any(|b| {
            b.command
                == Command::MoveToZone {
                    direction: Direction::Right,
                }
                && b.modifiers == vec![Modifier::Alt, Modifier::Shift]
                && b.key == Key::Char("l".into())
        });
        assert!(has_move_left, "should contain alt+shift+h = MoveToZone(Left)");
        assert!(
            has_move_right,
            "should contain alt+shift+l = MoveToZone(Right)"
        );
    }

    #[test]
    fn test_default_keybindings_contain_promote_to_primary() {
        let config = Config::default();
        let has_promote = config.keybindings.iter().any(|b| {
            b.command == Command::PromoteToPrimary
                && b.modifiers == vec![Modifier::Alt]
                && b.key == Key::Char("enter".into())
        });
        assert!(
            has_promote,
            "should contain alt+enter = PromoteToPrimary"
        );
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
        // 7 (j/k/h/l direction + shift+h/l move + enter promote)
        // + 2 (fullscreen, float)
        // + 9 (switch) + 9 (move) = 27
        assert_eq!(config.keybindings.len(), 27);
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
        assert_eq!(config.keybindings.len(), 27);
    }

    #[test]
    fn test_parse_empty_toml_uses_all_defaults() {
        let config = Config::from_toml("").unwrap();
        assert_eq!(config.gaps.inner, 8.0);
        assert_eq!(config.gaps.outer, 8.0);
        assert_eq!(config.keybindings.len(), 27);
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
        assert_eq!(config.keybindings.len(), 27);
    }

    #[test]
    fn test_parse_2col_layout() {
        let toml_str = r#"
[layouts.laptop-2col]
type = "columns"
ratios = [0.5, 0.5]
"#;
        let config = Config::from_toml(toml_str).unwrap();
        assert!(config.layouts.contains_key("laptop-2col"));
        let layout = &config.layouts["laptop-2col"];
        assert_eq!(layout.layout_type, "columns");
        assert_eq!(layout.ratios, vec![0.5, 0.5]);
    }

    #[test]
    fn test_parse_3col_layout() {
        let toml_str = r#"
[layouts.wide-3col]
type = "columns"
ratios = [0.25, 0.50, 0.25]
"#;
        let config = Config::from_toml(toml_str).unwrap();
        let layout = &config.layouts["wide-3col"];
        assert_eq!(layout.layout_type, "columns");
        assert_eq!(layout.ratios, vec![0.25, 0.50, 0.25]);
    }

    #[test]
    fn test_parse_composite_layout() {
        let toml_str = r#"
[layouts.main-stack]
type = "columns"
ratios = [0.6, 0.4]

[layouts.main-stack.splits.1]
type = "rows"
ratios = [0.5, 0.5]
"#;
        let config = Config::from_toml(toml_str).unwrap();
        let layout = &config.layouts["main-stack"];
        assert_eq!(layout.layout_type, "columns");
        assert!(layout.splits.contains_key("1"));
        let split = &layout.splits["1"];
        assert_eq!(split.layout_type, "rows");
        assert_eq!(split.ratios, vec![0.5, 0.5]);
    }

    #[test]
    fn test_parse_workspace_assignment() {
        let toml_str = r#"
[layouts.laptop-2col]
type = "columns"
ratios = [0.5, 0.5]

[workspaces.1]
layout = "laptop-2col"

[workspaces.2]
layout = "laptop-2col"
"#;
        let config = Config::from_toml(toml_str).unwrap();
        assert_eq!(config.workspaces["1"].layout, "laptop-2col");
        assert_eq!(config.workspaces["2"].layout, "laptop-2col");
    }

    #[test]
    fn test_build_workspace_layouts_2col() {
        let toml_str = r#"
[layouts.laptop-2col]
type = "columns"
ratios = [0.5, 0.5]

[workspaces.1]
layout = "laptop-2col"
"#;
        let config = Config::from_toml(toml_str).unwrap();
        let layouts = config.build_workspace_layouts();
        assert!(layouts.contains_key(&1));
        match &layouts[&1] {
            WorkspaceLayout::Zone(_) => {} // expected
            WorkspaceLayout::Monocle(_) => panic!("expected zone layout, got monocle"),
        }
    }

    #[test]
    fn test_build_workspace_layouts_composite() {
        let toml_str = r#"
[layouts.main-stack]
type = "columns"
ratios = [0.6, 0.4]

[layouts.main-stack.splits.1]
type = "rows"
ratios = [0.5, 0.5]

[workspaces.1]
layout = "main-stack"
"#;
        let config = Config::from_toml(toml_str).unwrap();
        let layouts = config.build_workspace_layouts();
        match &layouts[&1] {
            WorkspaceLayout::Zone(_) => {} // expected: main + 2 stacked = 3 zones
            WorkspaceLayout::Monocle(_) => panic!("expected zone layout"),
        }
    }

    #[test]
    fn test_build_workspace_layouts_monocle_type() {
        let toml_str = r#"
[layouts.mono]
type = "monocle"

[workspaces.1]
layout = "mono"
"#;
        let config = Config::from_toml(toml_str).unwrap();
        let layouts = config.build_workspace_layouts();
        match &layouts[&1] {
            WorkspaceLayout::Monocle(_) => {} // expected
            WorkspaceLayout::Zone(_) => panic!("expected monocle layout"),
        }
    }

    #[test]
    fn test_unknown_layout_name_falls_back_monocle() {
        let toml_str = r#"
[workspaces.1]
layout = "nonexistent"
"#;
        let config = Config::from_toml(toml_str).unwrap();
        let layouts = config.build_workspace_layouts();
        match &layouts[&1] {
            WorkspaceLayout::Monocle(_) => {} // expected fallback
            WorkspaceLayout::Zone(_) => panic!("should fall back to monocle for unknown name"),
        }
    }

    /// C4: Unknown layout_type falls back to monocle.
    #[test]
    fn test_unknown_layout_type_falls_back_monocle() {
        let toml_str = r#"
[layouts.weird]
type = "bsp"
ratios = [0.5, 0.5]

[workspaces.1]
layout = "weird"
"#;
        let config = Config::from_toml(toml_str).unwrap();
        let layouts = config.build_workspace_layouts();
        match &layouts[&1] {
            WorkspaceLayout::Monocle(_) => {} // expected: unknown type -> monocle
            WorkspaceLayout::Zone(_) => panic!("unknown layout type should fall back to monocle"),
        }
    }

    #[test]
    fn test_ratio_normalization_in_config() {
        let toml_str = r#"
[layouts.uneven]
type = "columns"
ratios = [0.30, 0.30, 0.30]

[workspaces.1]
layout = "uneven"
"#;
        let config = Config::from_toml(toml_str).unwrap();
        let layouts = config.build_workspace_layouts();
        // Should build successfully with normalized ratios
        match &layouts[&1] {
            WorkspaceLayout::Zone(_) => {} // ratios normalized, zone built
            WorkspaceLayout::Monocle(_) => panic!("should normalize ratios and build zone"),
        }
    }

    #[test]
    fn test_empty_config_defaults_monocle() {
        let config = Config::default();
        let layouts = config.build_workspace_layouts();
        // No workspaces configured -> empty map (all default to monocle in WorkspaceManager)
        assert!(layouts.is_empty());
    }

    /// C5: Recursion depth limit -- nesting beyond max 4 levels is treated as leaf.
    #[test]
    fn test_recursion_depth_limit() {
        // Build a config with depth 6 via TOML
        let toml_str = r#"
[layouts.deep]
type = "columns"
ratios = [0.5, 0.5]

[layouts.deep.splits.1]
type = "rows"
ratios = [0.5, 0.5]

[layouts.deep.splits.1.splits.1]
type = "columns"
ratios = [0.5, 0.5]

[layouts.deep.splits.1.splits.1.splits.1]
type = "rows"
ratios = [0.5, 0.5]

[layouts.deep.splits.1.splits.1.splits.1.splits.1]
type = "columns"
ratios = [0.5, 0.5]

[workspaces.1]
layout = "deep"
"#;
        let config = Config::from_toml(toml_str).unwrap();
        let layouts = config.build_workspace_layouts();
        // Should not panic -- depth > 4 treated as leaf
        match &layouts[&1] {
            WorkspaceLayout::Zone(_) => {} // built with truncated depth
            WorkspaceLayout::Monocle(_) => panic!("should still build as zone (truncated)"),
        }
    }

    #[test]
    fn test_zone_test_config_parses_all_layouts() {
        let toml_str = include_str!("../../../config/test-zone-layouts.toml");
        let config = Config::from_toml(toml_str).unwrap();
        assert_eq!(config.layouts.len(), 4);
        assert!(config.layouts.contains_key("laptop-2col"));
        assert!(config.layouts.contains_key("wide-3col"));
        assert!(config.layouts.contains_key("main-stack"));
        assert!(config.layouts.contains_key("two-row"));
        let layouts = config.build_workspace_layouts();
        // All 4 workspaces should get zone layouts
        for ws_id in 1..=4u8 {
            match &layouts[&ws_id] {
                WorkspaceLayout::Zone(_) => {}
                WorkspaceLayout::Monocle(_) => panic!("workspace {ws_id} should be zone layout"),
            }
        }
    }

    #[test]
    fn test_rows_layout_type() {
        let toml_str = r#"
[layouts.two-rows]
type = "rows"
ratios = [0.6, 0.4]

[workspaces.1]
layout = "two-rows"
"#;
        let config = Config::from_toml(toml_str).unwrap();
        let layouts = config.build_workspace_layouts();
        match &layouts[&1] {
            WorkspaceLayout::Zone(_) => {}
            WorkspaceLayout::Monocle(_) => panic!("expected zone layout for rows"),
        }
    }
}
