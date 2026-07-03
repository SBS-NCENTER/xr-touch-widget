use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub network: NetworkConfig,
    pub targets: Vec<Target>,
    pub buttons: Vec<ButtonDef>,
    pub appearance: AppearanceConfig,
    pub window: WindowConfig,
    pub layout: LayoutConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct NetworkConfig {
    pub ue_port: u16,
    pub listen_port: u16,
    pub heartbeat_interval_ms: u64,
    pub heartbeat_timeout_misses: u32,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            ue_port: 8000,
            listen_port: 8001,
            heartbeat_interval_ms: 1000,
            heartbeat_timeout_misses: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Target {
    pub name: String,
    pub ip: String,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ButtonDef {
    pub label: String,
    pub graphic_id: String,
    #[serde(rename = "type", default = "default_button_type")]
    pub button_type: String,
}

fn default_button_type() -> String {
    "trigger".into()
}

/// Palette look-and-feel, tunable from the settings window (D9, 2026-07-03).
/// All fields `serde(default)` so a config.toml written before D9 existed
/// still loads cleanly, with these values filled in from spec defaults.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AppearanceConfig {
    pub bg_opacity: f64,
    pub button_opacity: f64,
    pub accent: String,
    pub bg_tint: String,
    /// Give the most-recently-pressed button a heavier font-weight until
    /// another button is pressed (D12, 2026-07-03). Runtime-only on the UI
    /// side — the palette never writes `lastPressedId` back into config, it
    /// only reads this flag to decide whether to render the emphasis at all.
    pub highlight_last: bool,
}

impl Default for AppearanceConfig {
    fn default() -> Self {
        Self {
            bg_opacity: 0.55,
            button_opacity: 0.07,
            accent: "#4da3ff".into(),
            bg_tint: "#141820".into(),
            highlight_last: false,
        }
    }
}

/// Palette window size, saved when edit-mode resize (D8, 2026-07-03) ends
/// and re-applied on the next launch. All fields `serde(default)` so a
/// config.toml written before D8 existed still loads cleanly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        // Tall-narrow to suit the default vertical button column with the
        // control cluster on top (Task 8b, 2026-07-03).
        Self {
            width: 240,
            height: 400,
        }
    }
}

/// Button-grid layout (D11, 2026-07-03): `horizontal`/`vertical` are the two
/// checkboxes in the settings UI, `cols`/`rows` the accompanying counts.
/// - horizontal only → single row, `cols`/`rows` ignored.
/// - vertical only → single column, `cols`/`rows` ignored.
/// - both → CSS grid with `cols` columns, filled row-first; `rows` is a
///   MINIMUM (grid grows extra rows automatically so no button is ever
///   hidden when there are more buttons than `cols` x `rows` slots).
/// - neither → falls back to the horizontal-only single row.
/// All fields `serde(default)` so a config.toml written before D11 existed
/// still loads cleanly.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct LayoutConfig {
    pub horizontal: bool,
    pub vertical: bool,
    pub cols: u32,
    pub rows: u32,
}

impl Default for LayoutConfig {
    fn default() -> Self {
        // Default palette is vertical (buttons stack as a single N×1 column;
        // Task 8b, 2026-07-03). cols/rows are ignored in vertical-only mode
        // but kept sane so a both-checked config still has usable numbers.
        Self {
            horizontal: false,
            vertical: true,
            cols: 3,
            rows: 2,
        }
    }
}

/// Result of loading config: the app must always get a usable Config
/// (spec §8 — start with defaults instead of dying).
#[derive(Debug)]
pub enum LoadOutcome {
    Loaded,
    MissingUsedDefault,
    ParseErrorUsedDefault(String),
}

pub fn load(path: &Path) -> (Config, LoadOutcome) {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return (Config::default(), LoadOutcome::MissingUsedDefault),
    };
    match toml::from_str::<Config>(&text) {
        Ok(c) => (c, LoadOutcome::Loaded),
        Err(e) => (
            Config::default(),
            LoadOutcome::ParseErrorUsedDefault(e.to_string()),
        ),
    }
}

pub fn save(path: &Path, config: &Config) -> std::io::Result<()> {
    let text = toml::to_string_pretty(config).expect("Config is always serializable");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_has_spec_defaults() {
        let c = Config::default();
        assert_eq!(c.network.ue_port, 8000);
        assert_eq!(c.network.listen_port, 8001);
        assert_eq!(c.network.heartbeat_interval_ms, 1000);
        assert_eq!(c.network.heartbeat_timeout_misses, 3);
        assert!(c.targets.is_empty());
        assert!(c.buttons.is_empty());
    }

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut c = Config::default();
        c.targets.push(Target {
            name: "XR-1".into(),
            ip: "192.168.0.10".into(),
            active: true,
        });
        c.buttons.push(ButtonDef {
            label: "그래픽 A".into(),
            graphic_id: "lower_third_a".into(),
            button_type: "trigger".into(),
        });
        c.appearance = AppearanceConfig {
            bg_opacity: 0.4,
            button_opacity: 0.12,
            accent: "#ff8800".into(),
            bg_tint: "#202020".into(),
            highlight_last: false,
        };
        c.window = WindowConfig {
            width: 900,
            height: 120,
        };
        save(&path, &c).unwrap();
        let (loaded, outcome) = load(&path);
        assert_eq!(loaded, c);
        assert!(matches!(outcome, LoadOutcome::Loaded));
    }

    #[test]
    fn appearance_and_window_have_spec_defaults() {
        let a = AppearanceConfig::default();
        assert_eq!(a.bg_opacity, 0.55);
        assert_eq!(a.button_opacity, 0.07);
        assert_eq!(a.accent, "#4da3ff");
        assert_eq!(a.bg_tint, "#141820");

        let w = WindowConfig::default();
        assert_eq!(w.width, 240);
        assert_eq!(w.height, 400);
    }

    #[test]
    fn missing_appearance_and_window_sections_fall_back_to_defaults() {
        // Simulates a config.toml written before D8/D9 existed: no
        // [appearance] or [window] section at all.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
                [network]
                ue_port = 8000
                listen_port = 8001
                heartbeat_interval_ms = 1000
                heartbeat_timeout_misses = 3

                [[targets]]
                name = "XR-1"
                ip = "192.168.0.10"
                active = true
            "#,
        )
        .unwrap();
        let (c, outcome) = load(&path);
        assert_eq!(c.appearance, AppearanceConfig::default());
        assert_eq!(c.window, WindowConfig::default());
        // Legacy TOML with no [window] resolves to the new tall-narrow default.
        assert_eq!(c.window.width, 240);
        assert_eq!(c.window.height, 400);
        assert!(matches!(outcome, LoadOutcome::Loaded));
    }

    #[test]
    fn missing_file_falls_back_to_default() {
        let dir = tempfile::tempdir().unwrap();
        let (c, outcome) = load(&dir.path().join("nope.toml"));
        assert_eq!(c, Config::default());
        assert!(matches!(outcome, LoadOutcome::MissingUsedDefault));
    }

    #[test]
    fn corrupt_file_falls_back_to_default_with_reason() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "this is {{ not toml").unwrap();
        let (c, outcome) = load(&path);
        assert_eq!(c, Config::default());
        assert!(matches!(outcome, LoadOutcome::ParseErrorUsedDefault(_)));
    }

    #[test]
    fn button_type_defaults_to_trigger_when_absent() {
        let toml_str = r#"
            [[buttons]]
            label = "A"
            graphic_id = "a"
        "#;
        let c: Config = toml::from_str(toml_str).unwrap();
        assert_eq!(c.buttons[0].button_type, "trigger");
    }

    // --- Task 8b (D11/D12): [layout] section + appearance.highlight_last ---

    #[test]
    fn layout_config_has_spec_defaults() {
        // Default palette is vertical (Task 8b): a single button column.
        let l = LayoutConfig::default();
        assert!(!l.horizontal);
        assert!(l.vertical);
        assert_eq!(l.cols, 3);
        assert_eq!(l.rows, 2);
    }

    #[test]
    fn appearance_highlight_last_defaults_to_false() {
        assert!(!AppearanceConfig::default().highlight_last);
    }

    #[test]
    fn missing_layout_section_falls_back_to_default() {
        // Simulates a config.toml written before D11 existed: no [layout]
        // section at all.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r#"
                [network]
                ue_port = 8000
                listen_port = 8001
                heartbeat_interval_ms = 1000
                heartbeat_timeout_misses = 3
            "#,
        )
        .unwrap();
        let (c, outcome) = load(&path);
        assert_eq!(c.layout, LayoutConfig::default());
        // Legacy TOML with no [layout] resolves to the new vertical default.
        assert!(!c.layout.horizontal);
        assert!(c.layout.vertical);
        assert!(matches!(outcome, LoadOutcome::Loaded));
    }

    #[test]
    fn missing_highlight_last_falls_back_to_default_false() {
        // Simulates a config.toml written before D12 existed: [appearance]
        // present with its D9 fields, but no highlight_last line.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(
            &path,
            r##"
                [appearance]
                bg_opacity = 0.4
                button_opacity = 0.12
                accent = "#ff8800"
                bg_tint = "#202020"
            "##,
        )
        .unwrap();
        let (c, outcome) = load(&path);
        assert!(!c.appearance.highlight_last);
        assert_eq!(c.appearance.bg_opacity, 0.4);
        assert!(matches!(outcome, LoadOutcome::Loaded));
    }

    #[test]
    fn layout_and_highlight_last_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut c = Config::default();
        c.layout = LayoutConfig {
            horizontal: false,
            vertical: true,
            cols: 4,
            rows: 5,
        };
        c.appearance.highlight_last = true;
        save(&path, &c).unwrap();
        let (loaded, outcome) = load(&path);
        assert_eq!(loaded, c);
        assert!(!loaded.layout.horizontal);
        assert!(loaded.layout.vertical);
        assert_eq!(loaded.layout.cols, 4);
        assert_eq!(loaded.layout.rows, 5);
        assert!(loaded.appearance.highlight_last);
        assert!(matches!(outcome, LoadOutcome::Loaded));
    }
}
