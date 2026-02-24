use crate::domain::config::{KeystrokeConfig as Config, Position};
use anyhow::Result;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, CssProvider};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use tracing::info;

fn generate_overlay_css(
    keystroke_font_family: &str,
    keystroke_font_size: f64,
    keystroke_opacity: f64,
    bubble_font_family: &str,
    bubble_font_size: f64,
    bubble_opacity: f64,
) -> String {
    let safe_ks_family = keystroke_font_family.replace('"', "\\\"");
    let safe_bubble_family = bubble_font_family.replace('"', "\\\"");

    let overlay = format!(
        include_str!("../../style/overlay.css"),
        keystroke_font_family = safe_ks_family,
        keystroke_font_size = keystroke_font_size,
        keystroke_opacity = keystroke_opacity,
        bubble_font_family = safe_bubble_family,
        bubble_font_size = bubble_font_size,
        bubble_opacity = bubble_opacity
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

    window.set_namespace("keystroke");

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
    let css = generate_overlay_css(
        &config.font_family,
        config.font_size,
        config.opacity,
        &config.bubble.font_family,
        config.bubble.font_size,
        config.bubble.opacity,
    );
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

    window.set_namespace("keystroke-bubble");

    window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::None);

    window.set_anchor(Edge::Top, false);
    window.set_anchor(Edge::Bottom, true);
    window.set_anchor(Edge::Left, true);
    window.set_anchor(Edge::Right, false);

    window.set_margin(Edge::Top, config.margin);
    window.set_margin(Edge::Bottom, config.margin + 100);
    window.set_margin(Edge::Left, config.margin);
    window.set_margin(Edge::Right, config.margin);

    window.set_exclusive_zone(0);

    window.add_css_class("bubble-window");

    info!("Created bubble window");

    Ok(window)
}
