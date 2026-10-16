use gtk4_layer_shell::Edge;
use serde::{Deserialize, Serialize};

const DEFAULT_DISPLAY_TIMEOUT_MS: u64 = 2000;
const DEFAULT_BUBBLE_TIMEOUT_MS: u64 = 10000;
const DEFAULT_MAX_KEYS: usize = 5;
const DEFAULT_MAX_COMBO_KEYS: usize = 4;
const DEFAULT_MAX_WORD_LENGTH: usize = 20;
const DEFAULT_MARGIN: i32 = 20;

pub trait Validate {
    fn validate(&mut self);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DisplayMode {
    #[default]
    Keystroke,
    Bubble,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Position {
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    Center,
    MiddleRight,
    BottomLeft,
    #[default]
    BottomCenter,
    BottomRight,
}

impl Position {
    #[must_use]
    pub fn layer_shell_edges(self) -> Vec<(Edge, bool)> {
        match self {
            Position::TopLeft => vec![
                (Edge::Top, true),
                (Edge::Left, true),
                (Edge::Bottom, false),
                (Edge::Right, false),
            ],
            Position::TopCenter => vec![
                (Edge::Top, true),
                (Edge::Left, false),
                (Edge::Bottom, false),
                (Edge::Right, false),
            ],
            Position::TopRight => vec![
                (Edge::Top, true),
                (Edge::Left, false),
                (Edge::Bottom, false),
                (Edge::Right, true),
            ],
            Position::MiddleLeft => vec![
                (Edge::Top, false),
                (Edge::Left, true),
                (Edge::Bottom, false),
                (Edge::Right, false),
            ],
            Position::Center => vec![
                (Edge::Top, false),
                (Edge::Left, false),
                (Edge::Bottom, false),
                (Edge::Right, false),
            ],
            Position::MiddleRight => vec![
                (Edge::Top, false),
                (Edge::Left, false),
                (Edge::Bottom, false),
                (Edge::Right, true),
            ],
            Position::BottomLeft => vec![
                (Edge::Top, false),
                (Edge::Left, true),
                (Edge::Bottom, true),
                (Edge::Right, false),
            ],
            Position::BottomCenter => vec![
                (Edge::Top, false),
                (Edge::Left, false),
                (Edge::Bottom, true),
                (Edge::Right, false),
            ],
            Position::BottomRight => vec![
                (Edge::Top, false),
                (Edge::Left, false),
                (Edge::Bottom, true),
                (Edge::Right, true),
            ],
        }
    }
}

/// How Hibiki registers its global hotkeys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GlobalShortcutsBackend {
    /// Use the portal when available, otherwise raw device capture.
    #[default]
    Auto,
    /// Only use the XDG Desktop Portal `GlobalShortcuts` interface.
    Portal,
    /// Only use raw device capture (evdev), ignoring the portal.
    Evdev,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SuperKeyIconMode {
    #[default]
    Hibiki,
    Symbolic,
    Icon,
    /// Must be a PNG/WebP with an alpha channel so the key background shows through.
    Image,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct SuperKeyIconConfig {
    pub mode: SuperKeyIconMode,
    pub icon_name: Option<String>,
    pub image_path: Option<String>,
}

impl Validate for SuperKeyIconConfig {
    fn validate(&mut self) {
        if self.icon_name.as_deref().is_some_and(str::is_empty) {
            self.icon_name = None;
        }
        if self.image_path.as_deref().is_some_and(str::is_empty) {
            self.image_path = None;
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct SoundConfig {
    pub enabled: bool,
    pub volume: f32,
    pub sound_pack: String,
    pub folder: Option<String>,
}

pub const DEFAULT_SOUND_PACK: &str = "cherrymx-blue-abs";

impl Validate for SoundConfig {
    fn validate(&mut self) {
        self.volume = self.volume.clamp(0.0, 1.0);
        if self.folder.as_deref().is_some_and(str::is_empty) {
            self.folder = None;
        }
    }
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            volume: 1.0,
            sound_pack: DEFAULT_SOUND_PACK.to_string(),
            folder: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct BubbleConfig {
    pub font_family: String,
    pub font_size: f64,
    pub color: String,
    pub background: Option<String>,
    pub foreground: Option<String>,
    pub position: Position,
    pub draggable: bool,
    pub hotkey: String,
    pub timeout_ms: u64,
    pub opacity: f64,
    pub corner_radius: f64,
}

impl Validate for BubbleConfig {
    fn validate(&mut self) {
        self.font_size = self.font_size.clamp(1.0, 10.0);

        self.timeout_ms = self.timeout_ms.clamp(100, 30000);
        self.opacity = self.opacity.clamp(0.0, 1.0);
        self.corner_radius = self.corner_radius.clamp(0.0, 1.0);
    }
}

impl Default for BubbleConfig {
    fn default() -> Self {
        Self {
            font_family: "Sans".to_string(),
            font_size: 1.0,
            color: "#3584e4".to_string(),
            background: None,
            foreground: None,
            position: Position::TopRight,
            draggable: false,
            hotkey: "<Shift><Control>b".to_string(),
            timeout_ms: DEFAULT_BUBBLE_TIMEOUT_MS,
            opacity: 1.0,
            corner_radius: 0.36,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)]
pub struct KeystrokeConfig {
    pub display_mode: DisplayMode,

    pub position: Position,

    pub display_timeout_ms: u64,

    pub max_keys: usize,

    pub margin: i32,

    pub show_modifiers: bool,

    pub separate_duplicate_keys: bool,

    pub combine_modifier_keys: bool,

    pub max_combo_keys: usize,

    pub unify_words: bool,

    pub max_word_length: usize,

    pub backspace_edits_word: bool,

    pub all_keyboards: bool,

    pub font_scale: f64,

    pub opacity: f64,

    pub font_family: String,

    pub font_size: f64,

    pub keystroke_theme: String,

    pub background: Option<String>,

    pub foreground: Option<String>,

    pub keystroke_draggable: bool,

    pub keystroke_hotkey: String,

    pub pause_hotkey: String,

    pub toggle_focus_hotkey: String,

    pub toggle_mode_hotkey: String,

    pub global_shortcuts: GlobalShortcutsBackend,

    pub auto_detect_layout: bool,

    pub keyboard_layout: Option<String>,

    pub super_key_icon: SuperKeyIconConfig,

    pub bubble: BubbleConfig,

    pub sound: SoundConfig,

    pub show_visual: bool,

    pub corner_radius: f64,
}

impl Validate for KeystrokeConfig {
    fn validate(&mut self) {
        self.display_timeout_ms = self.display_timeout_ms.clamp(100, 30000);

        self.max_keys = self.max_keys.clamp(1, 50);

        self.max_combo_keys = self.max_combo_keys.clamp(2, 12);

        self.max_word_length = self.max_word_length.clamp(1, 50);

        self.font_scale = self.font_scale.clamp(1.0, 10.0);

        self.opacity = self.opacity.clamp(0.0, 1.0);

        self.font_size = self.font_size.clamp(1.0, 10.0);

        self.margin = self.margin.clamp(-100, 1000);

        self.corner_radius = self.corner_radius.clamp(0.0, 1.0);

        self.bubble.validate();
        self.sound.validate();
        self.super_key_icon.validate();
    }
}

impl Default for KeystrokeConfig {
    fn default() -> Self {
        Self {
            display_mode: DisplayMode::Keystroke,
            position: Position::BottomCenter,
            display_timeout_ms: DEFAULT_DISPLAY_TIMEOUT_MS,
            max_keys: DEFAULT_MAX_KEYS,
            margin: DEFAULT_MARGIN,
            show_modifiers: true,
            separate_duplicate_keys: true,
            combine_modifier_keys: false,
            max_combo_keys: DEFAULT_MAX_COMBO_KEYS,
            unify_words: false,
            max_word_length: DEFAULT_MAX_WORD_LENGTH,
            backspace_edits_word: false,
            all_keyboards: true,
            font_scale: 1.0,
            opacity: 1.0,
            font_family: "Sans".to_string(),
            font_size: 1.2,
            keystroke_theme: "system".to_string(),
            background: None,
            foreground: None,
            keystroke_draggable: false,
            keystroke_hotkey: "<Shift><Control>k".to_string(),
            pause_hotkey: "<Control>p".to_string(),
            toggle_focus_hotkey: "<Control>b".to_string(),
            toggle_mode_hotkey: "<Shift><Control>t".to_string(),
            global_shortcuts: GlobalShortcutsBackend::Auto,
            auto_detect_layout: true,
            keyboard_layout: None,
            super_key_icon: SuperKeyIconConfig::default(),
            bubble: BubbleConfig::default(),
            sound: SoundConfig::default(),
            show_visual: true,
            corner_radius: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sound_config_validation() {
        let mut config = SoundConfig {
            enabled: true,
            volume: 1.5,
            sound_pack: "test".to_string(),
            folder: None,
        };
        config.validate();
        assert_eq!(config.volume, 1.0);

        config.volume = -0.5;
        config.validate();
        assert_eq!(config.volume, 0.0);
    }

    #[test]
    fn test_sound_defaults_and_visual_enabled() {
        let config = KeystrokeConfig::default();
        assert!(!config.sound.enabled);
        assert_eq!(config.sound.sound_pack, "cherrymx-blue-abs");
        assert!(config.show_visual);
    }

    #[test]
    fn test_bubble_config_validation() {
        let mut config = BubbleConfig {
            font_size: 0.0,
            timeout_ms: 50,
            opacity: 1.5,
            corner_radius: 1.5,
            ..Default::default()
        };
        config.validate();
        assert_eq!(config.font_size, 1.0);
        assert_eq!(config.timeout_ms, 100);
        assert_eq!(config.opacity, 1.0);
        assert_eq!(config.corner_radius, 1.0);

        config.font_size = 15.0;
        config.timeout_ms = 40000;
        config.opacity = -0.5;
        config.corner_radius = -0.5;
        config.validate();
        assert_eq!(config.font_size, 10.0);
        assert_eq!(config.timeout_ms, 30000);
        assert_eq!(config.opacity, 0.0);
        assert_eq!(config.corner_radius, 0.0);
    }

    #[test]
    fn test_color_config_defaults_to_auto() {
        let config = KeystrokeConfig::default();
        assert!(config.background.is_none());
        assert!(config.foreground.is_none());
        assert!(config.bubble.background.is_none());
        assert!(config.bubble.foreground.is_none());
    }

    #[test]
    fn test_color_config_toml_roundtrip() {
        let config = KeystrokeConfig {
            background: Some("#1e1e2e".to_string()),
            foreground: Some("#cdd6f4".to_string()),
            bubble: BubbleConfig {
                background: Some("#181825".to_string()),
                foreground: Some("#cdd6f4".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let toml = toml::to_string_pretty(&config).unwrap();
        let parsed: KeystrokeConfig = toml::from_str(&toml).unwrap();

        assert_eq!(parsed.background.as_deref(), Some("#1e1e2e"));
        assert_eq!(parsed.foreground.as_deref(), Some("#cdd6f4"));
        assert_eq!(parsed.bubble.background.as_deref(), Some("#181825"));
        assert_eq!(parsed.bubble.foreground.as_deref(), Some("#cdd6f4"));
    }

    #[test]
    fn test_super_key_icon_defaults_to_hibiki_logo() {
        let config = KeystrokeConfig::default();
        assert_eq!(config.super_key_icon.mode, SuperKeyIconMode::Hibiki);
        assert!(config.super_key_icon.icon_name.is_none());
        assert!(config.super_key_icon.image_path.is_none());
    }

    #[test]
    fn test_super_key_icon_toml_roundtrip() {
        let config = KeystrokeConfig {
            super_key_icon: SuperKeyIconConfig {
                mode: SuperKeyIconMode::Symbolic,
                icon_name: Some("start-here-symbolic".to_string()),
                image_path: None,
            },
            ..Default::default()
        };

        let toml = toml::to_string_pretty(&config).unwrap();
        let parsed: KeystrokeConfig = toml::from_str(&toml).unwrap();

        assert_eq!(parsed.super_key_icon.mode, SuperKeyIconMode::Symbolic);
        assert_eq!(
            parsed.super_key_icon.icon_name.as_deref(),
            Some("start-here-symbolic")
        );
    }

    #[test]
    fn test_super_key_icon_validation_clears_blank_strings() {
        let mut config = SuperKeyIconConfig {
            mode: SuperKeyIconMode::Image,
            icon_name: Some(String::new()),
            image_path: Some(String::new()),
        };
        config.validate();
        assert!(config.icon_name.is_none());
        assert!(config.image_path.is_none());
    }

    #[test]
    fn test_max_word_length_validation() {
        let mut config = KeystrokeConfig {
            max_word_length: 0,
            ..Default::default()
        };
        config.validate();
        assert_eq!(config.max_word_length, 1);

        config.max_word_length = 999;
        config.validate();
        assert_eq!(config.max_word_length, 50);
    }

    #[test]
    fn test_toggle_mode_hotkey_default_and_roundtrip() {
        let config = KeystrokeConfig::default();
        assert_eq!(config.toggle_mode_hotkey, "<Shift><Control>t");
        assert_eq!(config.global_shortcuts, GlobalShortcutsBackend::Auto);
        assert!(!config.backspace_edits_word);

        let toml = toml::to_string_pretty(&config).unwrap();
        let parsed: KeystrokeConfig = toml::from_str(&toml).unwrap();
        assert_eq!(parsed.toggle_mode_hotkey, config.toggle_mode_hotkey);
        assert_eq!(parsed.max_word_length, config.max_word_length);
        assert_eq!(parsed.global_shortcuts, config.global_shortcuts);
        assert_eq!(parsed.backspace_edits_word, config.backspace_edits_word);
    }

    #[test]
    fn test_global_shortcuts_backend_roundtrip() {
        let config = KeystrokeConfig {
            global_shortcuts: GlobalShortcutsBackend::Evdev,
            ..Default::default()
        };
        let toml = toml::to_string_pretty(&config).unwrap();
        assert!(toml.contains("global_shortcuts = \"evdev\""));
        let parsed: KeystrokeConfig = toml::from_str(&toml).unwrap();
        assert_eq!(parsed.global_shortcuts, GlobalShortcutsBackend::Evdev);
    }

    #[test]
    fn test_keystroke_config_validation() {
        let mut config = KeystrokeConfig {
            display_timeout_ms: 50,
            max_keys: 0,
            font_scale: 0.0,
            opacity: 1.5,
            font_size: 0.0,
            margin: -200,
            corner_radius: 2.0,
            ..Default::default()
        };
        config.validate();
        assert_eq!(config.display_timeout_ms, 100);
        assert_eq!(config.max_keys, 1);
        assert_eq!(config.font_scale, 1.0);
        assert_eq!(config.opacity, 1.0);
        assert_eq!(config.font_size, 1.0);
        assert_eq!(config.margin, -100);
        assert_eq!(config.corner_radius, 1.0);

        config.display_timeout_ms = 40000;
        config.max_keys = 100;
        config.font_scale = 15.0;
        config.opacity = -0.5;
        config.font_size = 15.0;
        config.margin = 1500;
        config.corner_radius = -1.0;
        config.validate();
        assert_eq!(config.display_timeout_ms, 30000);
        assert_eq!(config.max_keys, 50);
        assert_eq!(config.font_scale, 10.0);
        assert_eq!(config.opacity, 0.0);
        assert_eq!(config.font_size, 10.0);
        assert_eq!(config.margin, 1000);
        assert_eq!(config.corner_radius, 0.0);
    }
}
