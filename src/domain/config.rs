use gtk4_layer_shell::Edge;
use serde::{Deserialize, Serialize};

const DEFAULT_DISPLAY_TIMEOUT_MS: u64 = 2000;
const DEFAULT_BUBBLE_TIMEOUT_MS: u64 = 10000;
const DEFAULT_MAX_KEYS: usize = 5;
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AudioConfig {
    pub enabled: bool,
    pub volume: f32,
    pub sound_pack: String,
}

impl Validate for AudioConfig {
    fn validate(&mut self) {
        self.volume = self.volume.clamp(0.0, 1.0);
    }
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            volume: 1.0,
            sound_pack: "cherrymx-blue-abs".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct BubbleConfig {
    pub font_family: String,
    pub font_size: f64,
    pub color: String,
    pub audio: AudioConfig,
    pub position: Position,
    pub draggable: bool,
    pub hotkey: String,
    pub timeout_ms: u64,
    pub opacity: f64,
}

impl Validate for BubbleConfig {
    fn validate(&mut self) {
        self.font_size = self.font_size.clamp(1.0, 10.0);

        self.timeout_ms = self.timeout_ms.clamp(100, 30000);
        self.opacity = self.opacity.clamp(0.0, 1.0);

        self.audio.validate();
    }
}

impl Default for BubbleConfig {
    fn default() -> Self {
        Self {
            font_family: "Sans".to_string(),
            font_size: 1.0,
            color: "#3584e4".to_string(),
            audio: AudioConfig::default(),
            position: Position::TopRight,
            draggable: false,
            hotkey: "<Shift><Control>b".to_string(),
            timeout_ms: DEFAULT_BUBBLE_TIMEOUT_MS,
            opacity: 1.0,
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

    pub all_keyboards: bool,

    pub font_scale: f64,

    pub opacity: f64,

    pub font_family: String,

    pub font_size: f64,

    pub keystroke_theme: String,

    pub keystroke_draggable: bool,

    pub keystroke_hotkey: String,

    pub pause_hotkey: String,

    pub toggle_focus_hotkey: String,

    pub auto_detect_layout: bool,

    pub keyboard_layout: Option<String>,

    pub bubble: BubbleConfig,

    pub audio: AudioConfig,
}

impl Validate for KeystrokeConfig {
    fn validate(&mut self) {
        self.display_timeout_ms = self.display_timeout_ms.clamp(100, 30000);

        self.max_keys = self.max_keys.clamp(1, 50);

        self.font_scale = self.font_scale.clamp(1.0, 10.0);

        self.opacity = self.opacity.clamp(0.0, 1.0);

        self.font_size = self.font_size.clamp(1.0, 10.0);

        self.margin = self.margin.clamp(-100, 1000);

        self.bubble.validate();
        self.audio.validate();
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
            all_keyboards: true,
            font_scale: 1.0,
            opacity: 1.0,
            font_family: "Sans".to_string(),
            font_size: 1.2,
            keystroke_theme: "system".to_string(),
            keystroke_draggable: false,
            keystroke_hotkey: "<Shift><Control>k".to_string(),
            pause_hotkey: "<Control>p".to_string(),
            toggle_focus_hotkey: "<Control>b".to_string(),
            auto_detect_layout: true,
            keyboard_layout: None,
            bubble: BubbleConfig::default(),
            audio: AudioConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_config_validation() {
        let mut config = AudioConfig {
            enabled: true,
            volume: 1.5,
            sound_pack: "test".to_string(),
        };
        config.validate();
        assert_eq!(config.volume, 1.0);

        config.volume = -0.5;
        config.validate();
        assert_eq!(config.volume, 0.0);
    }

    #[test]
    fn test_bubble_config_validation() {
        let mut config = BubbleConfig {
            font_size: 0.0,
            timeout_ms: 50,
            opacity: 1.5,
            ..Default::default()
        };
        config.validate();
        assert_eq!(config.font_size, 1.0);
        assert_eq!(config.timeout_ms, 100);
        assert_eq!(config.opacity, 1.0);

        config.font_size = 15.0;
        config.timeout_ms = 40000;
        config.opacity = -0.5;
        config.validate();
        assert_eq!(config.font_size, 10.0);
        assert_eq!(config.timeout_ms, 30000);
        assert_eq!(config.opacity, 0.0);
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
            ..Default::default()
        };
        config.validate();
        assert_eq!(config.display_timeout_ms, 100);
        assert_eq!(config.max_keys, 1);
        assert_eq!(config.font_scale, 1.0);
        assert_eq!(config.opacity, 1.0);
        assert_eq!(config.font_size, 1.0);
        assert_eq!(config.margin, -100);

        config.display_timeout_ms = 40000;
        config.max_keys = 100;
        config.font_scale = 15.0;
        config.opacity = -0.5;
        config.font_size = 15.0;
        config.margin = 1500;
        config.validate();
        assert_eq!(config.display_timeout_ms, 30000);
        assert_eq!(config.max_keys, 50);
        assert_eq!(config.font_scale, 10.0);
        assert_eq!(config.opacity, 0.0);
        assert_eq!(config.font_size, 10.0);
        assert_eq!(config.margin, 1000);
    }
}
