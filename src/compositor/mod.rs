pub mod hyprland;
pub mod niri;
pub mod sway;
pub mod umbriel;

use std::env;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Compositor {
    Hyprland,

    Sway,

    Niri,

    Umbriel,

    River,

    Dwl,

    Labwc,

    Wayfire,

    Unknown,
}

impl Compositor {
    #[must_use]
    pub const fn supports_layout_query(&self) -> bool {
        matches!(
            self,
            Self::Hyprland | Self::Sway | Self::Niri | Self::Umbriel
        )
    }

    #[must_use]
    pub const fn supports_layout_events(&self) -> bool {
        matches!(
            self,
            Self::Hyprland | Self::Sway | Self::Niri | Self::Umbriel
        )
    }

    #[must_use]
    pub const fn detection_env_var(&self) -> Option<&'static str> {
        match self {
            Self::Hyprland => Some("HYPRLAND_INSTANCE_SIGNATURE"),
            Self::Sway => Some("SWAYSOCK"),
            Self::Niri => Some("NIRI_SOCKET"),
            Self::Umbriel => Some("UMBRIEL_SOCKET"),
            Self::Wayfire => Some("WAYFIRE_SOCKET"),
            _ => None,
        }
    }
}

impl fmt::Display for Compositor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Hyprland => write!(f, "Hyprland"),
            Self::Sway => write!(f, "Sway"),
            Self::Niri => write!(f, "Niri"),
            Self::Umbriel => write!(f, "Umbriel"),
            Self::River => write!(f, "River"),
            Self::Dwl => write!(f, "dwl"),
            Self::Labwc => write!(f, "Labwc"),
            Self::Wayfire => write!(f, "Wayfire"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyboardLayouts {
    pub names: Vec<String>,

    pub current_idx: usize,
}

impl KeyboardLayouts {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            names: Vec::new(),
            current_idx: 0,
        }
    }

    #[must_use]
    pub fn current_name(&self) -> Option<&str> {
        self.names.get(self.current_idx).map(String::as_str)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.names.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutEvent {
    LayoutSwitched { name: String, index: usize },

    LayoutsChanged { layouts: KeyboardLayouts },
}

pub trait CompositorClient: Send + Sync {
    fn get_keyboard_layouts(&self) -> anyhow::Result<KeyboardLayouts>;

    fn is_available(&self) -> bool;
}

#[must_use]
pub fn detect() -> Compositor {
    if env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        return Compositor::Hyprland;
    }

    if env::var_os("SWAYSOCK").is_some() {
        return Compositor::Sway;
    }

    if env::var_os("NIRI_SOCKET").is_some() || env::var_os("NIRI_SOCKET_PATH").is_some() {
        return Compositor::Niri;
    }

    if env::var_os("UMBRIEL_SOCKET").is_some() {
        return Compositor::Umbriel;
    }

    if env::var_os("WAYFIRE_SOCKET").is_some() {
        return Compositor::Wayfire;
    }

    if let Ok(desktop) = env::var("XDG_CURRENT_DESKTOP") {
        let desktop_lower = desktop.to_lowercase();

        if desktop_lower.contains("river") {
            return Compositor::River;
        }
        if desktop_lower.contains("dwl") {
            return Compositor::Dwl;
        }
        if desktop_lower.contains("labwc") {
            return Compositor::Labwc;
        }

        if desktop_lower.contains("hyprland") {
            return Compositor::Hyprland;
        }
        if desktop_lower.contains("sway") {
            return Compositor::Sway;
        }
        if desktop_lower.contains("niri") {
            return Compositor::Niri;
        }
        if desktop_lower.contains("umbriel") {
            return Compositor::Umbriel;
        }
    }

    Compositor::Unknown
}

#[must_use]
pub fn create_client(compositor: Compositor) -> Option<Box<dyn CompositorClient>> {
    match compositor {
        Compositor::Hyprland => {
            hyprland::HyprlandClient::new().map(|c| Box::new(c) as Box<dyn CompositorClient>)
        }
        Compositor::Sway => {
            sway::SwayClient::new().map(|c| Box::new(c) as Box<dyn CompositorClient>)
        }
        Compositor::Niri => {
            niri::NiriClient::new().map(|c| Box::new(c) as Box<dyn CompositorClient>)
        }
        Compositor::Umbriel => {
            umbriel::UmbrielClient::new().map(|c| Box::new(c) as Box<dyn CompositorClient>)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyboard_layouts_default() {
        let layouts = KeyboardLayouts::default();
        assert!(layouts.is_empty());
        assert_eq!(layouts.len(), 0);
        assert!(layouts.current_name().is_none());
    }

    #[test]
    fn test_keyboard_layouts_with_data() {
        let layouts = KeyboardLayouts {
            names: vec!["English (US)".to_string(), "German".to_string()],
            current_idx: 1,
        };
        assert!(!layouts.is_empty());
        assert_eq!(layouts.len(), 2);
        assert_eq!(layouts.current_name(), Some("German"));
    }

    #[test]
    fn test_compositor_supports_layout_query() {
        assert!(Compositor::Hyprland.supports_layout_query());
        assert!(Compositor::Sway.supports_layout_query());
        assert!(Compositor::Niri.supports_layout_query());
        assert!(Compositor::Umbriel.supports_layout_query());
        assert!(!Compositor::River.supports_layout_query());
        assert!(!Compositor::Unknown.supports_layout_query());
    }

    #[test]
    fn test_compositor_display() {
        assert_eq!(format!("{}", Compositor::Hyprland), "Hyprland");
        assert_eq!(format!("{}", Compositor::Sway), "Sway");
        assert_eq!(format!("{}", Compositor::Niri), "Niri");
        assert_eq!(format!("{}", Compositor::Umbriel), "Umbriel");
    }
}
