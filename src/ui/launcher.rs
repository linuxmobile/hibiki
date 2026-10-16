use gtk4::gdk::Texture;
use gtk4::gdk_pixbuf::Pixbuf;
use gtk4::prelude::*;
use gtk4::Box as GtkBox;
use gtk4::{Align, Application, ApplicationWindow, Button, CssProvider, Image, Label, Orientation};
use std::rc::Rc;
use tracing::debug;

const LOGO_SVG: &[u8] = include_bytes!("../assets/logo-symbolic.svg");

const LAUNCHER_CSS: &str = include_str!("../../style/launcher.css");
const DEFAULTS_CSS: &str = include_str!("../../style/defaults.css");

pub use crate::domain::config::DisplayMode;
use crate::domain::config::KeystrokeConfig;
use crate::ui::settings::format_hotkey;

pub fn create_launcher_window(
    app: &Application,
    config: &KeystrokeConfig,
    on_select: impl Fn(DisplayMode) + 'static,
    on_settings: impl Fn() + 'static,
) -> ApplicationWindow {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Hibiki")
        .default_width(900)
        .default_height(780)
        .resizable(false)
        .build();

    apply_launcher_css(&window);
    window.add_css_class("launcher-window");

    let content = create_launcher_content(&window, config, on_select, on_settings);
    window.set_child(Some(&content));

    window
}

fn apply_launcher_css(window: &ApplicationWindow) {
    let provider = CssProvider::new();
    let css = format!("{}\n{}", DEFAULTS_CSS, LAUNCHER_CSS);
    provider.load_from_string(&css);

    let display = gtk4::prelude::WidgetExt::display(window);
    gtk4::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );
}

fn create_launcher_content(
    window: &ApplicationWindow,
    config: &KeystrokeConfig,
    on_select: impl Fn(DisplayMode) + 'static,
    on_settings: impl Fn() + 'static,
) -> GtkBox {
    let container = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .halign(Align::Center)
        .valign(Align::Center)
        .margin_top(48)
        .margin_bottom(48)
        .margin_start(48)
        .margin_end(48)
        .build();

    let logo_box = GtkBox::builder()
        .width_request(80)
        .height_request(80)
        .halign(Align::Center)
        .margin_bottom(16)
        .build();

    let (bg_color, fg_color) = {
        #[allow(deprecated)]
        let style_context = window.style_context();
        let fg = window.color();
        #[allow(deprecated)]
        let bg = style_context
            .lookup_color("theme_bg_color")
            .unwrap_or(gtk4::gdk::RGBA::new(0.12, 0.12, 0.12, 1.0));

        let to_hex = |rgba: gtk4::gdk::RGBA| {
            format!(
                "#{:02x}{:02x}{:02x}",
                (rgba.red() * 255.0) as u8,
                (rgba.green() * 255.0) as u8,
                (rgba.blue() * 255.0) as u8
            )
        };
        (to_hex(bg), to_hex(fg))
    };
    debug!("Logo colors: fg={}, bg={}", fg_color, bg_color);

    let svg_str = String::from_utf8_lossy(LOGO_SVG);
    let svg_with_color = svg_str
        .replace("currentColor", &fg_color)
        .replace("currentBackgroundColor", &bg_color);

    let stream = gtk4::gio::MemoryInputStream::from_bytes(&gtk4::glib::Bytes::from(
        svg_with_color.as_bytes(),
    ));

    let logo_image = if let Ok(pixbuf) = Pixbuf::from_stream_at_scale(
        &stream,
        80,
        80,
        true,
        Option::<&gtk4::gio::Cancellable>::None,
    ) {
        let texture = Texture::for_pixbuf(&pixbuf);
        Image::from_paintable(Some(&texture))
    } else {
        Image::from_icon_name("application-x-executable")
    };

    logo_image.set_pixel_size(80);
    logo_image.add_css_class("logo-icon");
    logo_box.append(&logo_image);

    container.append(&logo_box);

    let title = Label::new(Some("Hibiki"));
    title.add_css_class("header-title");
    container.append(&title);

    let subtitle = Label::new(Some(
        "Visualize your keystrokes and shortcuts — or just hear them.",
    ));
    subtitle.add_css_class("header-subtitle");
    container.append(&subtitle);

    let cards_box = GtkBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(24)
        .halign(Align::Center)
        .homogeneous(true)
        .build();

    let on_select = Rc::new(on_select);

    let keystroke_content = {
        let box_container = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(12)
            .halign(Align::Center)
            .valign(Align::Center)
            .build();

        let keys = ["Ctrl + J", "G", "I", "T"];
        for key in keys {
            let label = Label::new(Some(key));
            label.add_css_class("key-cap");
            box_container.append(&label);
        }
        box_container
    };

    let key_card = create_card(
        "Keystrokes",
        "Visualize shortcuts on screen.",
        "preferences-desktop-keyboard-symbolic",
        keystroke_content,
        {
            let win = window.clone();
            let cb = on_select.clone();
            move || {
                debug!("Keystroke mode selected");
                win.set_visible(false);
                cb(DisplayMode::Keystroke);
            }
        },
    );
    cards_box.append(&key_card);

    let bubble_content = {
        let box_container = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(12)
            .halign(Align::Start)
            .valign(Align::Center)
            .build();

        let texts = [
            "3 years ago, I've started this channel to show my ricing.",
            "I had zero video skills at that time",
            "Then, it's has passed 100k subscribers",
        ];

        for text in texts {
            let label = Label::builder()
                .label(text)
                .wrap(true)
                .max_width_chars(25)
                .xalign(0.0)
                .css_classes(["chat-bubble"])
                .build();
            box_container.append(&label);
        }
        box_container
    };

    let bubble_card = create_card(
        "Bubble Text",
        "Display context as floating bubbles.",
        "chat-symbolic",
        bubble_content,
        {
            let win = window.clone();
            let cb = on_select.clone();
            move || {
                debug!("Bubble mode selected");
                win.set_visible(false);
                cb(DisplayMode::Bubble);
            }
        },
    );
    cards_box.append(&bubble_card);

    container.append(&cards_box);

    container.append(&create_keybinds_section(config));

    let pref_icon = Image::from_icon_name("emblem-system-symbolic");
    pref_icon.set_pixel_size(16);

    let pref_label = Label::new(Some("Preferences"));

    let pref_btn = Button::builder()
        .css_classes(["footer-button"])
        .halign(Align::Center)
        .build();

    let inner_pref_box = GtkBox::builder()
        .orientation(Orientation::Horizontal)
        .spacing(8)
        .build();
    inner_pref_box.append(&pref_icon);
    inner_pref_box.append(&pref_label);
    pref_btn.set_child(Some(&inner_pref_box));

    let window_clone = window.clone();
    pref_btn.connect_clicked(move |_| {
        debug!("Preferences button clicked");
        window_clone.set_visible(false);
        on_settings();
    });

    container.append(&pref_btn);

    container
}

fn create_keybinds_section(config: &KeystrokeConfig) -> GtkBox {
    let section = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .halign(Align::Center)
        .spacing(12)
        .margin_top(32)
        .build();

    let title = Label::new(Some("KEYBINDS"));
    title.add_css_class("keybind-title");
    title.set_halign(Align::Start);
    section.append(&title);

    let grid = gtk4::Grid::builder()
        .column_spacing(24)
        .row_spacing(10)
        .halign(Align::Center)
        .build();

    let bindings = [
        (
            config.toggle_mode_hotkey.as_str(),
            "Toggle keystroke / bubble",
        ),
        (config.toggle_focus_hotkey.as_str(), "Toggle focus (Bubble)"),
        ("Escape", "Lose focus (Bubble)"),
        (config.keystroke_hotkey.as_str(), "Switch to Keystroke"),
        (config.bubble.hotkey.as_str(), "Switch to Bubble"),
        (config.pause_hotkey.as_str(), "Pause / resume capture"),
    ];

    for (i, (accelerator, description)) in bindings.iter().enumerate() {
        let col = (i % 2) * 2;
        let row = i / 2;

        let chip = Label::new(Some(&format_hotkey(accelerator)));
        chip.add_css_class("keybind-chip");
        chip.set_halign(Align::Start);

        let desc = Label::new(Some(description));
        desc.add_css_class("keybind-desc");
        desc.set_halign(Align::Start);

        grid.attach(&chip, col as i32, row as i32, 1, 1);
        grid.attach(&desc, (col + 1) as i32, row as i32, 1, 1);
    }

    section.append(&grid);
    section
}

fn create_card<W: IsA<gtk4::Widget>>(
    title_text: &str,
    desc_text: &str,
    icon_name: &str,
    preview_content: W,
    callback: impl Fn() + 'static,
) -> Button {
    let button = Button::builder()
        .css_classes(["card-button"])
        .width_request(300)
        .build();

    let content_box = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .css_classes(["card-content"])
        .build();

    let top_row = GtkBox::builder()
        .orientation(Orientation::Horizontal)
        .margin_bottom(16)
        .build();

    let icon_wrapper = GtkBox::builder().css_classes(["card-icon-bg"]).build();
    let icon = Image::from_icon_name(icon_name);
    icon.set_pixel_size(24);
    icon_wrapper.append(&icon);

    let spacer = GtkBox::builder().hexpand(true).build();

    let arrow = Image::from_icon_name("go-next-symbolic");
    arrow.set_opacity(0.3);
    arrow.set_valign(Align::Center);

    top_row.append(&icon_wrapper);
    top_row.append(&spacer);
    top_row.append(&arrow);
    content_box.append(&top_row);

    let title = Label::builder()
        .label(title_text)
        .css_classes(["card-title"])
        .halign(Align::Start)
        .build();
    content_box.append(&title);

    let desc = Label::builder()
        .label(desc_text)
        .css_classes(["card-desc"])
        .halign(Align::Start)
        .wrap(true)
        .build();
    content_box.append(&desc);

    let preview_box = GtkBox::builder()
        .css_classes(["card-preview"])
        .height_request(220)
        .build();

    preview_content.set_valign(Align::Center);
    preview_content.set_halign(Align::Center);

    preview_box.append(&preview_content);
    content_box.append(&preview_box);

    button.set_child(Some(&content_box));
    button.connect_clicked(move |_| callback());

    button
}

pub fn show_launcher(window: &ApplicationWindow) {
    window.set_visible(true);
    window.present();
}
