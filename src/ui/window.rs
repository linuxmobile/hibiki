use crate::domain::config::{KeystrokeConfig as Config, Position};
use anyhow::Result;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, CssProvider};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use tracing::info;

fn escape_css_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
        .replace('\r', "")
}

/// Validate and normalize a user-supplied CSS color.
///
/// Only a conservative, injection-safe subset is accepted: hex colors
/// (`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`) and `rgb()`/`rgba()` functional
/// notation with numeric components. Anything else is rejected so it can never
/// break out of the declaration and inject arbitrary CSS.
fn sanitize_color(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }

    if let Some(hex) = value.strip_prefix('#') {
        let valid_len = matches!(hex.len(), 3 | 4 | 6 | 8);
        if valid_len && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Some(format!("#{}", hex.to_ascii_lowercase()));
        }
        return None;
    }

    let lower = value.to_ascii_lowercase();
    if let Some(body) = lower
        .strip_prefix("rgba(")
        .and_then(|s| s.strip_suffix(')'))
    {
        return sanitize_rgb_body(body, 4).map(|_| lower);
    }
    if let Some(body) = lower.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        return sanitize_rgb_body(body, 3).map(|_| lower);
    }

    None
}

/// Check that `rgb()`/`rgba()` arguments are made up of numeric components.
fn sanitize_rgb_body(body: &str, expected: usize) -> Option<()> {
    let parts: Vec<&str> = body.split(',').map(str::trim).collect();
    if parts.len() != expected {
        return None;
    }
    for part in parts {
        let numeric = part.strip_suffix('%').unwrap_or(part);
        if numeric.is_empty() {
            return None;
        }
        let mut seen_dot = false;
        for c in numeric.chars() {
            match c {
                '0'..='9' => {}
                '.' if !seen_dot => seen_dot = true,
                _ => return None,
            }
        }
    }
    Some(())
}

fn generate_overlay_css(config: &Config) -> String {
    let safe_ks_family = escape_css_string(&config.font_family);
    let safe_bubble_family = escape_css_string(&config.bubble.font_family);

    let keystroke_background = config
        .background
        .as_deref()
        .and_then(sanitize_color)
        .unwrap_or_else(|| format!("alpha(@theme_bg_color, {})", config.opacity));
    let keystroke_foreground = config
        .foreground
        .as_deref()
        .and_then(sanitize_color)
        .unwrap_or_else(|| "@card_fg_color".to_string());

    let bubble_background = config
        .bubble
        .background
        .as_deref()
        .and_then(sanitize_color)
        .unwrap_or_else(|| format!("alpha(@theme_bg_color, {})", config.bubble.opacity));
    let bubble_foreground = config
        .bubble
        .foreground
        .as_deref()
        .and_then(sanitize_color)
        .unwrap_or_else(|| "@card_fg_color".to_string());

    let keystroke_modifier_background = if config.background.is_some() {
        keystroke_background.clone()
    } else {
        format!("alpha(@accent_bg_color, {})", config.opacity)
    };
    let keystroke_modifier_foreground =
        if config.background.is_some() || config.foreground.is_some() {
            keystroke_foreground.clone()
        } else {
            "@accent_fg_color".to_string()
        };

    let ks_radius_px = config.corner_radius * 30.0;
    let ks_radius_str = format!("{:.1}px", ks_radius_px);

    let b_radius_px = config.bubble.corner_radius * 50.0;
    let b_radius_str = match config.bubble.position {
        Position::TopLeft => format!(
            "0px {:.1}px {:.1}px {:.1}px",
            b_radius_px, b_radius_px, b_radius_px
        ),
        Position::TopRight => format!(
            "{:.1}px 0px {:.1}px {:.1}px",
            b_radius_px, b_radius_px, b_radius_px
        ),
        Position::BottomRight => format!(
            "{:.1}px {:.1}px 0px {:.1}px",
            b_radius_px, b_radius_px, b_radius_px
        ),
        Position::BottomLeft => format!(
            "{:.1}px {:.1}px {:.1}px 0px",
            b_radius_px, b_radius_px, b_radius_px
        ),
        _ => format!("{:.1}px", b_radius_px),
    };

    let overlay = format!(
        include_str!("../../style/overlay.css"),
        keystroke_font_family = safe_ks_family,
        keystroke_font_size = config.font_size,
        keystroke_background = keystroke_background,
        keystroke_foreground = keystroke_foreground,
        keystroke_modifier_background = keystroke_modifier_background,
        keystroke_modifier_foreground = keystroke_modifier_foreground,
        keystroke_border_radius = ks_radius_str,
        bubble_font_family = safe_bubble_family,
        bubble_font_size = config.bubble.font_size,
        bubble_background = bubble_background,
        bubble_foreground = bubble_foreground,
        bubble_border_radius = b_radius_str
    );
    format!(
        "{}\n{}\n{}\n{}",
        include_str!("../../style/defaults.css"),
        include_str!("../../style/settings.css"),
        include_str!("../../style/bubble.css"),
        overlay
    )
}

pub fn create_window(app: &Application, config: &Config) -> Result<ApplicationWindow> {
    let window = ApplicationWindow::builder()
        .application(app)
        .decorated(false)
        .resizable(false)
        .build();

    window.init_layer_shell();

    window.set_layer(Layer::Overlay);

    window.set_namespace("hibiki");

    window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::None);

    for (edge, anchor) in config.position.layer_shell_edges() {
        window.set_anchor(edge, anchor);
    }

    window.set_margin(Edge::Top, config.margin);
    window.set_margin(Edge::Bottom, config.margin);
    window.set_margin(Edge::Left, config.margin);
    window.set_margin(Edge::Right, config.margin);

    window.set_exclusive_zone(0);

    window.add_css_class("keystroke-window");

    info!(
        "Created layer shell window at position {:?}",
        config.position
    );

    Ok(window)
}

pub fn update_css_provider(provider: &CssProvider, config: &Config) {
    let css = generate_overlay_css(config);
    provider.load_from_string(&css);
}

pub fn update_position(window: &ApplicationWindow, position: Position, margin: i32) {
    info!(
        "Updating window position to: {:?} with margin {}",
        position, margin
    );
    for (edge, anchor) in position.layer_shell_edges() {
        window.set_anchor(edge, anchor);
    }

    window.set_margin(Edge::Top, margin);
    window.set_margin(Edge::Bottom, margin);
    window.set_margin(Edge::Left, margin);
    window.set_margin(Edge::Right, margin);

    window.queue_resize();
}

pub fn create_bubble_window(app: &Application, config: &Config) -> Result<ApplicationWindow> {
    let window = ApplicationWindow::builder()
        .application(app)
        .decorated(false)
        .resizable(false)
        .build();

    window.init_layer_shell();

    window.set_layer(Layer::Overlay);

    window.set_namespace("hibiki-bubble");

    window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::None);

    for (edge, anchor) in config.bubble.position.layer_shell_edges() {
        window.set_anchor(edge, anchor);
    }

    window.set_margin(Edge::Top, config.margin);
    window.set_margin(Edge::Bottom, config.margin);
    window.set_margin(Edge::Left, config.margin);
    window.set_margin(Edge::Right, config.margin);

    window.set_exclusive_zone(0);

    window.add_css_class("bubble-window");

    info!("Created bubble window");

    Ok(window)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_css_string() {
        assert_eq!(escape_css_string("Sans"), "Sans");
        assert_eq!(
            escape_css_string("Font \"With\" Quotes"),
            "Font \\\"With\\\" Quotes"
        );
        assert_eq!(
            escape_css_string("Font\\With\\Backslashes"),
            "Font\\\\With\\\\Backslashes"
        );
        assert_eq!(
            escape_css_string("Font\nWith\nNewline"),
            "Font With Newline"
        );
        assert_eq!(
            escape_css_string("Injection\"; color: red;"),
            "Injection\\\"; color: red;"
        );
    }

    #[test]
    fn test_sanitize_color_accepts_valid_values() {
        assert_eq!(sanitize_color("#1e1e2e").as_deref(), Some("#1e1e2e"));
        assert_eq!(sanitize_color("#ABC").as_deref(), Some("#abc"));
        assert_eq!(sanitize_color("#CDd6F4").as_deref(), Some("#cdd6f4"));
        assert_eq!(
            sanitize_color("  #ff00ff80  ").as_deref(),
            Some("#ff00ff80")
        );
        assert_eq!(
            sanitize_color("rgb(30, 30, 46)").as_deref(),
            Some("rgb(30, 30, 46)")
        );
        assert_eq!(
            sanitize_color("rgba(30, 30, 46, 0.5)").as_deref(),
            Some("rgba(30, 30, 46, 0.5)")
        );
    }

    #[test]
    fn test_sanitize_color_rejects_injection() {
        assert!(sanitize_color("").is_none());
        assert!(sanitize_color("red").is_none());
        assert!(sanitize_color("#12345").is_none());
        assert!(sanitize_color("#zzzzzz").is_none());
        assert!(sanitize_color("#fff; } body { background: red").is_none());
        assert!(sanitize_color("url(evil)").is_none());
        assert!(sanitize_color("rgb(1, 2)").is_none());
        assert!(sanitize_color("rgb(1, 2, 3, 4)").is_none());
        assert!(sanitize_color("rgb(a, b, c)").is_none());
    }

    #[test]
    fn test_generate_overlay_css_uses_custom_colors() {
        let config = Config {
            background: Some("#1e1e2e".to_string()),
            foreground: Some("#cdd6f4".to_string()),
            bubble: crate::domain::config::BubbleConfig {
                background: Some("rgba(10, 10, 10, 0.8)".to_string()),
                foreground: Some("#ffffff".to_string()),
                ..Default::default()
            },
            ..Default::default()
        };

        let css = generate_overlay_css(&config);

        assert!(css.contains("background-color: #1e1e2e;"));
        assert!(css.contains("color: #cdd6f4;"));
        assert!(css.contains("background-color: rgba(10, 10, 10, 0.8);"));
        assert!(css.contains("color: #ffffff;"));
    }

    #[test]
    fn test_generate_overlay_css_falls_back_on_invalid_colors() {
        let config = Config {
            background: Some("red; } body { }".to_string()),
            foreground: Some("not-a-color".to_string()),
            ..Default::default()
        };

        let css = generate_overlay_css(&config);

        assert!(!css.contains("red; } body"));
        assert!(css.contains("alpha(@theme_bg_color"));
        assert!(css.contains("color: @card_fg_color;"));
    }
}
