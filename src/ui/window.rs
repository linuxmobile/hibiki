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

fn generate_overlay_css(config: &Config) -> String {
    let safe_ks_family = escape_css_string(&config.font_family);
    let safe_bubble_family = escape_css_string(&config.bubble.font_family);

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
        keystroke_opacity = config.opacity,
        keystroke_border_radius = ks_radius_str,
        bubble_font_family = safe_bubble_family,
        bubble_font_size = config.bubble.font_size,
        bubble_opacity = config.bubble.opacity,
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

pub fn create_mouse_cursor_window(app: &Application, config: &Config) -> Result<ApplicationWindow> {
    let window = ApplicationWindow::builder()
        .application(app)
        .decorated(false)
        .resizable(false)
        .build();

    window.init_layer_shell();

    window.set_layer(Layer::Overlay);

    window.set_namespace("hibiki-mouse");

    window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::None);

    // Anchor to same position as keystroke window
    for (edge, anchor) in config.position.layer_shell_edges() {
        window.set_anchor(edge, anchor);
    }

    // Offset mouse bubbles 70px UP from keystroke window based on anchor position
    let vertical_offset = config.position.vertical_offset_direction();
    let top_margin = if let Some((Edge::Top, dir)) = vertical_offset {
        config.margin + (dir * 70)
    } else {
        config.margin
    };
    let bottom_margin = if let Some((Edge::Bottom, dir)) = vertical_offset {
        config.margin + (dir * 70)
    } else {
        config.margin
    };

    window.set_margin(Edge::Top, top_margin);
    window.set_margin(Edge::Bottom, bottom_margin);
    window.set_margin(Edge::Left, config.margin);
    window.set_margin(Edge::Right, config.margin);

    window.set_exclusive_zone(0);

    window.add_css_class("mouse-cursor-window");

    info!("Created mouse cursor window");

    Ok(window)
}

pub fn update_mouse_window_position(window: &ApplicationWindow, x: i32, y: i32, offset_x: i32, offset_y: i32) {
    let display = gtk4::prelude::WidgetExt::display(window);

    if let Some(surface) = window.surface() {
        if let Some(monitor) = display.monitor_at_surface(&surface) {
            let geometry = monitor.geometry();

            let rel_x = x - geometry.x();
            let rel_y = y - geometry.y();

            let margin_left = rel_x + offset_x;
            let margin_top = rel_y + offset_y;

            window.set_margin(Edge::Left, margin_left.max(0));
            window.set_margin(Edge::Top, margin_top.max(0));
        }
    }
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
}
