//! Data-driven CLI theming via YAML skin definitions.
//!
//! A `Skin` controls all visual aspects of the terminal UI — spinner faces,
//! tool result colors, diff coloring, box drawing characters, and branding.
//! Built-in skins ship as embedded YAML; users can drop custom skins into
//! `~/.eidolon/skins/` and switch at runtime with `/skin <name>`.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

/// ANSI 256-color code or named color.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(untagged)]
pub enum SkinColor {
    /// ANSI 256-color index (0-255).
    Ansi256(u8),
    /// Named color string ("red", "green", "cyan", etc.).
    Named(String),
}

impl SkinColor {
    /// Convert to an ANSI escape sequence for foreground color.
    #[must_use]
    pub fn fg(&self) -> String {
        match self {
            Self::Ansi256(code) => format!("\x1b[38;5;{code}m"),
            Self::Named(name) => match name.as_str() {
                "red" => "\x1b[31m".to_string(),
                "green" => "\x1b[32m".to_string(),
                "yellow" => "\x1b[33m".to_string(),
                "blue" => "\x1b[34m".to_string(),
                "magenta" => "\x1b[35m".to_string(),
                "cyan" => "\x1b[36m".to_string(),
                "white" => "\x1b[37m".to_string(),
                "gray" | "grey" => "\x1b[90m".to_string(),
                "bright_red" => "\x1b[91m".to_string(),
                "bright_green" => "\x1b[92m".to_string(),
                "bright_yellow" => "\x1b[93m".to_string(),
                "bright_blue" => "\x1b[94m".to_string(),
                "bright_cyan" => "\x1b[96m".to_string(),
                _ => "\x1b[0m".to_string(),
            },
        }
    }
}

/// Reset escape code.
pub const RESET: &str = "\x1b[0m";
/// Bold escape code.
pub const BOLD: &str = "\x1b[1m";
/// Dim escape code.
pub const DIM: &str = "\x1b[2m";

/// Complete skin definition loaded from YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Skin {
    /// Display name for the skin.
    pub name: String,
    /// Short description.
    pub description: String,

    // ── Tool result colors ──────────────────────────────────────────
    /// Color for success icons (✓).
    pub success: SkinColor,
    /// Color for error icons (✗).
    pub error: SkinColor,
    /// Color for muted/secondary text (paths, labels).
    pub muted: SkinColor,
    /// Color for diff removed lines.
    pub diff_remove: SkinColor,
    /// Color for diff added lines.
    pub diff_add: SkinColor,
    /// Color for diff hunk headers (@@).
    pub diff_hunk: SkinColor,
    /// Color for tool name in result boxes.
    pub tool_name: SkinColor,
    /// Color for file write operations.
    pub file_write: SkinColor,
    /// Color for file edit operations.
    pub file_edit: SkinColor,

    // ── Spinner ─────────────────────────────────────────────────────
    /// Spinner frame characters.
    pub spinner_frames: Vec<String>,
    /// Color for the active spinner.
    pub spinner_color: SkinColor,
    /// Color for the done spinner.
    pub spinner_done_color: SkinColor,

    // ── Box drawing ─────────────────────────────────────────────────
    /// Top-left corner for tool call boxes.
    pub box_top_left: String,
    /// Top-right corner.
    pub box_top_right: String,
    /// Bottom-left corner.
    pub box_bottom_left: String,
    /// Horizontal line.
    pub box_horizontal: String,
    /// Vertical line.
    pub box_vertical: String,
    /// Color for box borders.
    pub box_color: SkinColor,

    // ── Branding ────────────────────────────────────────────────────
    /// Prompt symbol shown before user input.
    pub prompt_symbol: String,
}

impl Default for Skin {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            description: "Eidolon default theme".to_string(),
            success: SkinColor::Ansi256(70),
            error: SkinColor::Ansi256(203),
            muted: SkinColor::Ansi256(245),
            diff_remove: SkinColor::Ansi256(203),
            diff_add: SkinColor::Ansi256(70),
            diff_hunk: SkinColor::Named("cyan".to_string()),
            tool_name: SkinColor::Ansi256(245),
            file_write: SkinColor::Named("bright_green".to_string()),
            file_edit: SkinColor::Named("bright_yellow".to_string()),
            spinner_frames: vec![
                "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏",
            ]
            .into_iter()
            .map(String::from)
            .collect(),
            spinner_color: SkinColor::Named("blue".to_string()),
            spinner_done_color: SkinColor::Named("green".to_string()),
            box_top_left: "╭".to_string(),
            box_top_right: "╮".to_string(),
            box_bottom_left: "╰".to_string(),
            box_horizontal: "─".to_string(),
            box_vertical: "│".to_string(),
            box_color: SkinColor::Ansi256(245),
            prompt_symbol: "> ".to_string(),
        }
    }
}

impl Skin {
    /// Load a skin from a YAML file.
    pub fn from_yaml_file(path: &Path) -> Result<Self, String> {
        let contents =
            fs::read_to_string(path).map_err(|e| format!("failed to read skin file: {e}"))?;
        Self::from_yaml(&contents)
    }

    /// Parse a skin from a YAML string.
    pub fn from_yaml(yaml: &str) -> Result<Self, String> {
        serde_yaml::from_str(yaml).map_err(|e| format!("failed to parse skin YAML: {e}"))
    }

    /// Serialize this skin to YAML.
    #[must_use]
    pub fn to_yaml(&self) -> String {
        serde_yaml::to_string(self).unwrap_or_default()
    }
}

// ── Built-in skins ──────────────────────────────────────────────────────────

const BUILTIN_MONO: &str = r#"
name: mono
description: "Monochrome grayscale theme"
success: 252
error: 240
muted: 242
diff_remove: 240
diff_add: 252
diff_hunk: 248
tool_name: 242
file_write: 252
file_edit: 248
spinner_frames: ["◐", "◓", "◑", "◒"]
spinner_color: 248
spinner_done_color: 252
box_color: 240
prompt_symbol: "$ "
"#;

const BUILTIN_SLATE: &str = r#"
name: slate
description: "Cool blue-grey theme"
success: 109
error: 167
muted: 103
diff_remove: 167
diff_add: 109
diff_hunk: 67
tool_name: 103
file_write: 109
file_edit: 179
spinner_frames: ["◜", "◠", "◝", "◞", "◡", "◟"]
spinner_color: 67
spinner_done_color: 109
box_color: 60
prompt_symbol: "› "
"#;

const BUILTIN_EMBER: &str = r#"
name: ember
description: "Warm amber and crimson theme"
success: 214
error: 196
muted: 180
diff_remove: 196
diff_add: 214
diff_hunk: 208
tool_name: 180
file_write: 214
file_edit: 220
spinner_frames: ["🔥", "✨", "🔥", "✨"]
spinner_color: 208
spinner_done_color: 214
box_color: 130
prompt_symbol: "⟫ "
"#;

/// Returns all built-in skins.
#[must_use]
pub fn builtin_skins() -> BTreeMap<String, Skin> {
    let mut skins = BTreeMap::new();
    skins.insert("default".to_string(), Skin::default());
    if let Ok(skin) = Skin::from_yaml(BUILTIN_MONO) {
        skins.insert("mono".to_string(), skin);
    }
    if let Ok(skin) = Skin::from_yaml(BUILTIN_SLATE) {
        skins.insert("slate".to_string(), skin);
    }
    if let Ok(skin) = Skin::from_yaml(BUILTIN_EMBER) {
        skins.insert("ember".to_string(), skin);
    }
    skins
}

/// Discover user-installed skins from `~/.eidolon/skins/`.
#[must_use]
pub fn user_skins_dir() -> Option<PathBuf> {
    std::env::var_os("EIDOLON_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".eidolon")))
        .or_else(|| std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join(".eidolon")))
        .map(|base| base.join("skins"))
}

/// Load all available skins (built-in + user).
#[must_use]
pub fn all_skins() -> BTreeMap<String, Skin> {
    let mut skins = builtin_skins();
    if let Some(dir) = user_skins_dir() {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "yaml" || ext == "yml") {
                    if let Ok(skin) = Skin::from_yaml_file(&path) {
                        skins.insert(skin.name.clone(), skin);
                    }
                }
            }
        }
    }
    skins
}

/// Thread-safe active skin holder. Switch at runtime via `set_active`.
#[derive(Clone)]
pub struct SkinManager {
    active: Arc<Mutex<Skin>>,
}

impl SkinManager {
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: Arc::new(Mutex::new(Skin::default())),
        }
    }

    #[must_use]
    pub fn with_skin(skin: Skin) -> Self {
        Self {
            active: Arc::new(Mutex::new(skin)),
        }
    }

    /// Get a snapshot of the current skin.
    #[must_use]
    pub fn current(&self) -> Skin {
        self.active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// Switch to a different skin by name (looks up built-in + user skins).
    pub fn set_active(&self, name: &str) -> Result<(), String> {
        let skins = all_skins();
        let skin = skins
            .get(name)
            .ok_or_else(|| {
                let available: Vec<_> = skins.keys().map(String::as_str).collect();
                format!("unknown skin '{name}'. Available: {}", available.join(", "))
            })?
            .clone();
        *self
            .active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = skin;
        Ok(())
    }

    /// List names of all available skins.
    #[must_use]
    pub fn available_names(&self) -> Vec<String> {
        all_skins().keys().cloned().collect()
    }
}

impl Default for SkinManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_skin_round_trips_through_yaml() {
        let skin = Skin::default();
        let yaml = skin.to_yaml();
        let parsed = Skin::from_yaml(&yaml).expect("should parse");
        assert_eq!(parsed.name, "default");
        assert_eq!(parsed.spinner_frames.len(), 10);
    }

    #[test]
    fn builtin_skins_all_parse() {
        let skins = builtin_skins();
        assert!(skins.contains_key("default"));
        assert!(skins.contains_key("mono"));
        assert!(skins.contains_key("slate"));
        assert!(skins.contains_key("ember"));
        assert_eq!(skins.len(), 4);
    }

    #[test]
    fn skin_color_produces_ansi_escapes() {
        let c = SkinColor::Ansi256(70);
        assert_eq!(c.fg(), "\x1b[38;5;70m");

        let c = SkinColor::Named("red".to_string());
        assert_eq!(c.fg(), "\x1b[31m");
    }

    #[test]
    fn skin_manager_switches_skins() {
        let manager = SkinManager::new();
        assert_eq!(manager.current().name, "default");

        manager.set_active("mono").expect("should switch");
        assert_eq!(manager.current().name, "mono");

        assert!(manager.set_active("nonexistent").is_err());
    }

    #[test]
    fn mono_skin_has_grayscale_values() {
        let skins = builtin_skins();
        let mono = &skins["mono"];
        assert_eq!(mono.spinner_frames.len(), 4);
        assert_eq!(mono.prompt_symbol, "$ ");
    }
}
