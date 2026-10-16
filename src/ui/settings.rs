use crate::application::config_service::ConfigService;
use crate::application::typography_service::TypographyService;
use crate::domain::config::{GlobalShortcutsBackend, KeystrokeConfig, Position, SuperKeyIconMode};
use crate::infrastructure::audio::SoundPackLoader;
use gtk4::prelude::*;
use gtk4::{
    glib, Adjustment, Align, Application, ApplicationWindow, Box as GtkBox, Button, ColorDialog,
    ColorDialogButton, CustomFilter, DropDown, EventControllerKey, FileDialog, FileFilter,
    FilterListModel, Grid, GridView, Image, Label, ListItemFactory, ListScrollFlags, ListView,
    Orientation, Overflow, Scale, ScrolledWindow, SearchEntry, SelectionModel,
    SignalListItemFactory, SingleSelection, Stack, StringList, Switch, ToggleButton,
};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

pub fn create_settings_window(
    app: &Application,
    config_service: ConfigService,
    typography_service: TypographyService,
) -> ApplicationWindow {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Settings")
        .default_width(850)
        .default_height(650)
        .resizable(true)
        .build();

    let main_box = GtkBox::new(Orientation::Horizontal, 0);

    let stack = Stack::new();
    stack.set_transition_type(gtk4::StackTransitionType::Crossfade);
    stack.set_hexpand(true);
    stack.set_vexpand(true);

    let sidebar = gtk4::StackSidebar::builder()
        .stack(&stack)
        .width_request(200)
        .css_classes(vec!["background"])
        .build();

    let separator = gtk4::Separator::new(Orientation::Vertical);

    main_box.append(&sidebar);
    main_box.append(&separator);

    let keystroke_scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .hexpand(true)
        .build();
    let keystroke_content =
        create_keystroke_settings(&config_service, &typography_service, &window);
    keystroke_scroll.set_child(Some(&keystroke_content));
    stack.add_titled(&keystroke_scroll, Some("keystroke"), "Keystrokes");

    let bubble_scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .hexpand(true)
        .build();
    let bubble_content = create_bubble_settings(&config_service, &typography_service, &window);
    bubble_scroll.set_child(Some(&bubble_content));
    stack.add_titled(&bubble_scroll, Some("bubble"), "Bubbles");

    let sound_scroll = ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vscrollbar_policy(gtk4::PolicyType::Automatic)
        .vexpand(true)
        .hexpand(true)
        .build();
    let sound_content = create_sound_settings(&config_service, &window);
    sound_scroll.set_child(Some(&sound_content));
    stack.add_titled(&sound_scroll, Some("sound"), "Sounds");

    main_box.append(&stack);
    window.set_child(Some(&main_box));

    window
}

fn create_font_selector(
    parent: &GtkBox,
    typography_service: &TypographyService,
    config_service: &ConfigService,
    current_font: String,
    update_fn: impl Fn(&mut crate::domain::config::KeystrokeConfig, String) + 'static + Clone,
) {
    let type_section = GtkBox::new(Orientation::Vertical, 12);
    type_section.set_vexpand(true);

    let type_label = Label::builder()
        .label("TYPOGRAPHY")
        .css_classes(vec!["heading", "caption"])
        .halign(Align::Start)
        .build();
    type_section.append(&type_label);

    let search_entry = SearchEntry::builder()
        .placeholder_text("Search fonts...")
        .build();
    type_section.append(&search_entry);

    let font_scroll = ScrolledWindow::builder()
        .min_content_height(120)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .css_classes(vec!["font-list-frame"])
        .vexpand(false)
        .overflow(Overflow::Hidden)
        .build();

    let string_list = StringList::new(&[]);
    let filter = CustomFilter::new(move |_item| true);

    let filter_model = FilterListModel::new(Some(string_list.clone()), Some(filter.clone()));
    let selection_model = SingleSelection::new(Some(filter_model.clone()));

    let factory = SignalListItemFactory::new();

    factory.connect_setup(move |_, list_item| {
        let list_item = list_item
            .downcast_ref::<gtk4::ListItem>()
            .expect("list_item must be a gtk4::ListItem");
        let row_box = GtkBox::new(Orientation::Horizontal, 12);
        row_box.set_margin_start(12);
        row_box.set_margin_end(12);
        row_box.set_margin_top(8);
        row_box.set_margin_bottom(8);

        let label = Label::builder().xalign(0.0).hexpand(true).build();

        row_box.append(&label);

        list_item.set_child(Some(&row_box));
    });

    factory.connect_bind(move |_, list_item| {
        let list_item = list_item
            .downcast_ref::<gtk4::ListItem>()
            .expect("list_item must be a gtk4::ListItem");
        let string_obj = list_item
            .item()
            .and_downcast::<gtk4::StringObject>()
            .expect("item must be a gtk4::StringObject");
        let font_name = string_obj.string();

        let row_box = list_item
            .child()
            .and_downcast::<GtkBox>()
            .expect("child must be a GtkBox");
        let label = row_box
            .first_child()
            .and_downcast::<Label>()
            .expect("first_child must be a Label");

        label.set_label(&font_name);
    });

    let list_view = ListView::new(Some(selection_model.clone()), Some(factory));
    list_view.add_css_class("navigation-sidebar");

    let service_c = typography_service.clone();
    let list_c = string_list.clone();
    let selection_c = selection_model.clone();
    let current_font_startup = current_font.clone();
    let list_view_weak = list_view.downgrade();

    list_view.connect_map(move |list_view| {
        if let Some(model) = list_view.model() {
            if let Some(selection_model) = model.downcast_ref::<SingleSelection>() {
                let selected = selection_model.selected();
                if selected != gtk4::INVALID_LIST_POSITION {
                    list_view.scroll_to(
                        selected,
                        ListScrollFlags::FOCUS | ListScrollFlags::SELECT,
                        None,
                    );

                    let list_view_weak = list_view.downgrade();
                    glib::timeout_add_local_once(
                        std::time::Duration::from_millis(100),
                        move || {
                            if let Some(list_view) = list_view_weak.upgrade() {
                                list_view.scroll_to(
                                    selected,
                                    ListScrollFlags::FOCUS | ListScrollFlags::SELECT,
                                    None,
                                );
                            }
                        },
                    );
                }
            }
        }
    });

    glib::MainContext::default().spawn_local(async move {
        match service_c.get_system_fonts().await {
            Ok(fonts) => {
                let str_refs: Vec<&str> = fonts.iter().map(|s| s.as_str()).collect();
                list_c.splice(0, 0, &str_refs);

                if let Some(idx) = fonts.iter().position(|f| *f == current_font_startup) {
                    selection_c.set_selected(idx as u32);

                    let list_view_weak = list_view_weak.clone();
                    glib::timeout_add_local_once(std::time::Duration::from_millis(50), move || {
                        if let Some(list_view) = list_view_weak.upgrade() {
                            list_view.scroll_to(
                                idx as u32,
                                ListScrollFlags::FOCUS | ListScrollFlags::SELECT,
                                None,
                            );
                        }
                    });
                }
            }
            Err(e) => eprintln!("Failed to load fonts: {}", e),
        }
    });

    let filter_c = filter.clone();
    search_entry.connect_search_changed(move |entry| {
        let text = entry.text().to_lowercase();
        filter_c.set_filter_func(move |item| {
            let string_obj = item
                .downcast_ref::<gtk4::StringObject>()
                .expect("item must be a gtk4::StringObject");
            let font_name = string_obj.string().to_lowercase();
            font_name.contains(&text)
        });
    });

    let service_c = config_service.clone();

    selection_model.connect_selected_item_notify(move |model| {
        if let Some(item) = model.selected_item() {
            let string_obj = item
                .downcast::<gtk4::StringObject>()
                .expect("item must be a gtk4::StringObject");
            let name = string_obj.string();

            let mut cfg = service_c.get_config();
            update_fn(&mut cfg, name.to_string());
            let _ = service_c.update_config(cfg);
        }
    });

    font_scroll.set_child(Some(&list_view));
    type_section.append(&font_scroll);
    parent.append(&type_section);
}

fn create_slider_section(
    label_text: &str,
    initial_value: f64,
    min: f64,
    max: f64,
    step: f64,
    format_type: SliderFormat,
    on_change: impl Fn(f64) + 'static,
) -> GtkBox {
    let section = GtkBox::new(Orientation::Vertical, 4);
    let header = GtkBox::new(Orientation::Horizontal, 0);

    let label = Label::builder()
        .label(label_text)
        .css_classes(vec!["heading", "caption"])
        .halign(Align::Start)
        .hexpand(true)
        .build();

    let val_label_text = match format_type {
        SliderFormat::Percent => format!("{:.0}%", initial_value * 100.0),
        SliderFormat::Multiplier => format!("{:.1}x", initial_value),
        SliderFormat::Pixels => format!("{:.0}px", initial_value * 50.0),
    };

    let val_label = Label::builder()
        .label(&val_label_text)
        .css_classes(vec!["badge"])
        .build();

    header.append(&label);
    header.append(&val_label);
    section.append(&header);

    let (adj_val, adj_min, adj_max, adj_step) = match format_type {
        SliderFormat::Percent => (
            initial_value * 100.0,
            min * 100.0,
            max * 100.0,
            step * 100.0,
        ),
        SliderFormat::Multiplier => (initial_value, min, max, step),
        SliderFormat::Pixels => (
            initial_value * 100.0,
            min * 100.0,
            max * 100.0,
            step * 100.0,
        ),
    };

    let adjustment = Adjustment::new(adj_val, adj_min, adj_max, adj_step, adj_step * 10.0, 0.0);
    let scale = Scale::builder()
        .adjustment(&adjustment)
        .draw_value(false)
        .build();

    let val_label_c = val_label.clone();
    adjustment.connect_value_changed(move |adj| {
        let val = adj.value();
        let (display_text, actual_val) = match format_type {
            SliderFormat::Percent => (format!("{:.0}%", val), val / 100.0),
            SliderFormat::Multiplier => (format!("{:.1}x", val), val),
            SliderFormat::Pixels => (format!("{:.0}%", val), val / 100.0),
        };

        val_label_c.set_label(&display_text);
        on_change(actual_val);
    });

    section.append(&scale);
    section
}

enum SliderFormat {
    Percent,
    Multiplier,
    Pixels,
}

fn create_theme_section(current_theme: String, config_service: &ConfigService) -> GtkBox {
    let theme_section = GtkBox::new(Orientation::Vertical, 12);
    let theme_label = Label::builder()
        .label("THEME MODE")
        .css_classes(vec!["heading", "caption"])
        .halign(Align::Start)
        .build();
    theme_section.append(&theme_label);

    let theme_box = GtkBox::new(Orientation::Horizontal, 0);
    theme_box.add_css_class("linked");
    theme_box.set_halign(Align::Start);

    let theme_light = ToggleButton::builder().label("Light").build();
    let theme_dark = ToggleButton::builder().label("Dark").build();
    let theme_sys = ToggleButton::builder().label("System").build();
    theme_dark.set_group(Some(&theme_light));
    theme_sys.set_group(Some(&theme_light));

    match current_theme.as_str() {
        "light" => theme_light.set_active(true),
        "dark" => theme_dark.set_active(true),
        _ => theme_sys.set_active(true),
    }

    let service_c = config_service.clone();
    let theme_light_c = theme_light.clone();
    let theme_dark_c = theme_dark.clone();
    let update_theme = Rc::new(move || {
        let theme = if theme_light_c.is_active() {
            "light"
        } else if theme_dark_c.is_active() {
            "dark"
        } else {
            "system"
        };
        let mut cfg = service_c.get_config();
        if cfg.keystroke_theme != theme {
            cfg.keystroke_theme = theme.to_string();
            let _ = service_c.update_config(cfg);
        }
    });

    let u = update_theme.clone();
    theme_light.connect_toggled(move |_| u());
    let u = update_theme.clone();
    theme_dark.connect_toggled(move |_| u());
    let u = update_theme.clone();
    theme_sys.connect_toggled(move |_| u());

    theme_box.append(&theme_light);
    theme_box.append(&theme_dark);
    theme_box.append(&theme_sys);
    theme_section.append(&theme_box);
    theme_section
}

fn create_position_section(
    current_pos: Position,
    config_service: &ConfigService,
    is_bubble: bool,
) -> GtkBox {
    let pos_section = GtkBox::new(Orientation::Vertical, 12);
    pos_section.set_hexpand(true);
    pos_section.set_vexpand(true);
    let pos_label = Label::builder()
        .label("POSITION")
        .css_classes(vec!["heading", "caption"])
        .halign(Align::Start)
        .build();
    pos_section.append(&pos_label);

    let pos_grid = Grid::builder()
        .row_spacing(6)
        .column_spacing(6)
        .halign(Align::Fill)
        .valign(Align::Fill)
        .hexpand(true)
        .vexpand(true)
        .column_homogeneous(true)
        .row_homogeneous(true)
        .css_classes(vec!["position-grid"])
        .build();

    let positions = [
        (Position::TopLeft, 0, 0),
        (Position::TopCenter, 1, 0),
        (Position::TopRight, 2, 0),
        (Position::MiddleLeft, 0, 1),
        (Position::Center, 1, 1),
        (Position::MiddleRight, 2, 1),
        (Position::BottomLeft, 0, 2),
        (Position::BottomCenter, 1, 2),
        (Position::BottomRight, 2, 2),
    ];

    let mut first_btn: Option<ToggleButton> = None;

    for (pos, col, row) in positions {
        let btn = ToggleButton::builder()
            .hexpand(true)
            .vexpand(true)
            .css_classes(vec!["position-toggle"])
            .build();

        if pos == current_pos {
            btn.set_active(true);
        }

        if let Some(ref first) = first_btn {
            btn.set_group(Some(first));
        } else {
            first_btn = Some(btn.clone());
        }

        let service_c = config_service.clone();
        btn.connect_toggled(move |b| {
            if b.is_active() {
                let mut cfg = service_c.get_config();
                let current = if is_bubble {
                    cfg.bubble.position
                } else {
                    cfg.position
                };

                if current != pos {
                    if is_bubble {
                        cfg.bubble.position = pos;
                    } else {
                        cfg.position = pos;
                    }
                    let _ = service_c.update_config(cfg);
                }
            }
        });

        pos_grid.attach(&btn, col, row, 1, 1);
    }

    pos_section.append(&pos_grid);
    pos_section
}

fn rgba_to_hex(color: &gtk4::gdk::RGBA) -> String {
    let channel = |v: f32| (v * 255.0).round() as u8;
    let (r, g, b, a) = (
        channel(color.red()),
        channel(color.green()),
        channel(color.blue()),
        channel(color.alpha()),
    );
    if a == 255 {
        format!("#{r:02x}{g:02x}{b:02x}")
    } else {
        format!("#{r:02x}{g:02x}{b:02x}{a:02x}")
    }
}

fn parse_color(value: &str, fallback: gtk4::gdk::RGBA) -> gtk4::gdk::RGBA {
    gtk4::gdk::RGBA::parse(value).unwrap_or(fallback)
}

#[allow(deprecated)]
fn resolve_theme_color(widget: &impl IsA<gtk4::Widget>, name: &str) -> Option<gtk4::gdk::RGBA> {
    widget.style_context().lookup_color(name)
}

fn set_color_value(
    config: &mut KeystrokeConfig,
    is_bubble: bool,
    is_background: bool,
    value: Option<String>,
) {
    match (is_bubble, is_background) {
        (true, true) => config.bubble.background = value,
        (true, false) => config.bubble.foreground = value,
        (false, true) => config.background = value,
        (false, false) => config.foreground = value,
    }
}

fn create_color_row(
    title: &str,
    current: Option<String>,
    config_service: &ConfigService,
    is_bubble: bool,
    is_background: bool,
    theme_color: gtk4::gdk::RGBA,
) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 12);
    row.add_css_class("settings-element");

    let label = Label::builder()
        .label(title)
        .hexpand(true)
        .xalign(0.0)
        .build();
    row.append(&label);

    let value_label = Label::builder()
        .label(current.clone().unwrap_or_else(|| "Auto".to_string()))
        .css_classes(vec!["caption", "dim-label"])
        .build();
    row.append(&value_label);

    let fallback = theme_color;
    let initial = current
        .as_deref()
        .map_or(fallback, |c| parse_color(c, fallback));

    let dialog = ColorDialog::builder()
        .title(format!("Choose {title} Color"))
        .with_alpha(true)
        .build();
    let button = ColorDialogButton::builder()
        .dialog(&dialog)
        .rgba(&initial)
        .valign(Align::Center)
        .build();
    row.append(&button);

    let reset = Button::builder()
        .icon_name("edit-clear-symbolic")
        .tooltip_text("Reset to theme")
        .valign(Align::Center)
        .build();
    row.append(&reset);

    let suppress = Rc::new(Cell::new(false));

    let service_c = config_service.clone();
    let value_c = value_label.clone();
    let suppress_c = Rc::clone(&suppress);
    button.connect_rgba_notify(move |btn| {
        if suppress_c.get() {
            return;
        }
        let hex = rgba_to_hex(&btn.rgba());
        let mut cfg = service_c.get_config();
        set_color_value(&mut cfg, is_bubble, is_background, Some(hex.clone()));
        let _ = service_c.update_config(cfg);
        value_c.set_label(&hex);
    });

    let service_c = config_service.clone();
    let value_c = value_label.clone();
    let button_c = button.clone();
    let suppress_c = Rc::clone(&suppress);
    reset.connect_clicked(move |_| {
        let mut cfg = service_c.get_config();
        set_color_value(&mut cfg, is_bubble, is_background, None);
        let _ = service_c.update_config(cfg);
        value_c.set_label("Auto");
        suppress_c.set(true);
        button_c.set_rgba(&fallback);
        suppress_c.set(false);
    });

    row
}

fn create_color_section(
    config_service: &ConfigService,
    is_bubble: bool,
    window: &ApplicationWindow,
) -> GtkBox {
    let config = config_service.get_config();
    let (background, foreground, opacity) = if is_bubble {
        (
            config.bubble.background.clone(),
            config.bubble.foreground.clone(),
            config.bubble.opacity,
        )
    } else {
        (
            config.background.clone(),
            config.foreground.clone(),
            config.opacity,
        )
    };

    let background_theme = resolve_theme_color(window, "theme_bg_color")
        .unwrap_or_else(|| gtk4::gdk::RGBA::new(0.13, 0.13, 0.13, 1.0))
        .with_alpha(opacity as f32);
    let foreground_theme = resolve_theme_color(window, "card_fg_color")
        .unwrap_or_else(|| gtk4::gdk::RGBA::new(1.0, 1.0, 1.0, 1.0));

    let section = GtkBox::new(Orientation::Vertical, 12);

    let label = Label::builder()
        .label("COLORS")
        .css_classes(vec!["heading", "caption"])
        .halign(Align::Start)
        .build();
    section.append(&label);

    let rows = GtkBox::new(Orientation::Vertical, 8);
    rows.append(&create_color_row(
        "Background",
        background,
        config_service,
        is_bubble,
        true,
        background_theme,
    ));
    rows.append(&create_color_row(
        "Text",
        foreground,
        config_service,
        is_bubble,
        false,
        foreground_theme,
    ));
    section.append(&rows);

    section
}

fn mode_index(mode: SuperKeyIconMode) -> u32 {
    match mode {
        SuperKeyIconMode::Hibiki => 0,
        SuperKeyIconMode::Symbolic => 1,
        SuperKeyIconMode::Icon => 2,
        SuperKeyIconMode::Image => 3,
    }
}

fn mode_from_index(index: u32) -> SuperKeyIconMode {
    match index {
        1 => SuperKeyIconMode::Symbolic,
        2 => SuperKeyIconMode::Icon,
        3 => SuperKeyIconMode::Image,
        _ => SuperKeyIconMode::Hibiki,
    }
}

fn backend_index(backend: GlobalShortcutsBackend) -> u32 {
    match backend {
        GlobalShortcutsBackend::Auto => 0,
        GlobalShortcutsBackend::Portal => 1,
        GlobalShortcutsBackend::Evdev => 2,
    }
}

fn backend_from_index(index: u32) -> GlobalShortcutsBackend {
    match index {
        1 => GlobalShortcutsBackend::Portal,
        2 => GlobalShortcutsBackend::Evdev,
        _ => GlobalShortcutsBackend::Auto,
    }
}

fn icons_for_mode<'a>(
    mode: SuperKeyIconMode,
    all: &'a [String],
    symbolic: &'a [String],
) -> &'a [String] {
    match mode {
        SuperKeyIconMode::Symbolic => symbolic,
        SuperKeyIconMode::Icon => all,
        SuperKeyIconMode::Hibiki | SuperKeyIconMode::Image => &[],
    }
}

/// Files without an alpha channel are rejected.
fn load_preview_texture(path: &str) -> Option<gtk4::gdk::Texture> {
    let pixbuf = gtk4::gdk_pixbuf::Pixbuf::from_file(path).ok()?;
    if !pixbuf.has_alpha() {
        return None;
    }
    Some(gtk4::gdk::Texture::for_pixbuf(&pixbuf))
}

fn create_super_key_icon_section(
    config_service: &ConfigService,
    window: &ApplicationWindow,
) -> GtkBox {
    let config = config_service.get_config();
    let current = config.super_key_icon.clone();

    let section = GtkBox::new(Orientation::Vertical, 12);

    let header = GtkBox::new(Orientation::Horizontal, 12);
    let title = Label::builder()
        .label("SUPER KEY ICON")
        .css_classes(vec!["heading", "caption"])
        .halign(Align::Start)
        .hexpand(true)
        .build();
    header.append(&title);

    let preview = Image::new();
    preview.set_pixel_size(24);
    preview.set_valign(Align::Center);
    header.append(&preview);
    section.append(&header);

    let dropdown = DropDown::from_strings(&["Hibiki logo", "Symbolic", "Icons", "Image"]);
    dropdown.set_halign(Align::Start);
    dropdown.set_selected(mode_index(current.mode));
    section.append(&dropdown);

    let hint = Label::builder()
        .label(
            "Symbolic and Icons use your current icon theme. Image accepts a transparent \
             PNG or WebP file.",
        )
        .css_classes(vec!["caption", "dim-label"])
        .halign(Align::Start)
        .xalign(0.0)
        .wrap(true)
        .build();
    section.append(&hint);

    let picker_box = GtkBox::new(Orientation::Vertical, 8);

    let search_entry = SearchEntry::builder()
        .placeholder_text("Search icons...")
        .build();
    picker_box.append(&search_entry);

    let icon_scroll = ScrolledWindow::builder()
        .min_content_height(160)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .css_classes(vec!["font-list-frame"])
        .overflow(Overflow::Hidden)
        .build();

    let string_list = StringList::new(&[]);
    let filter = CustomFilter::new(move |_item| true);
    let filter_model = FilterListModel::new(Some(string_list.clone()), Some(filter.clone()));
    let selection_model = SingleSelection::new(Some(filter_model.clone()));

    let factory = SignalListItemFactory::new();
    factory.connect_setup(|_, list_item| {
        let list_item = list_item
            .downcast_ref::<gtk4::ListItem>()
            .expect("list_item must be a gtk4::ListItem");
        let image = Image::new();
        image.set_pixel_size(24);
        image.set_halign(Align::Center);
        image.set_valign(Align::Center);
        image.set_margin_top(8);
        image.set_margin_bottom(8);
        list_item.set_child(Some(&image));
    });

    factory.connect_bind(|_, list_item| {
        let list_item = list_item
            .downcast_ref::<gtk4::ListItem>()
            .expect("list_item must be a gtk4::ListItem");
        let string_obj = list_item
            .item()
            .and_downcast::<gtk4::StringObject>()
            .expect("item must be a gtk4::StringObject");
        let name = string_obj.string();

        let image = list_item
            .child()
            .and_downcast::<Image>()
            .expect("child must be a gtk4::Image");
        image.set_icon_name(Some(&name));
        image.set_tooltip_text(Some(&name));
    });

    let grid_view = GridView::new(Some(selection_model.clone()), Some(factory));
    grid_view.set_min_columns(6);
    grid_view.set_max_columns(10);
    grid_view.add_css_class("navigation-sidebar");
    icon_scroll.set_child(Some(&grid_view));
    picker_box.append(&icon_scroll);
    section.append(&picker_box);

    let image_box = GtkBox::new(Orientation::Horizontal, 8);
    let path_label = Label::builder()
        .label(
            current
                .image_path
                .clone()
                .unwrap_or_else(|| "No image selected".to_string()),
        )
        .css_classes(vec!["caption", "dim-label"])
        .ellipsize(gtk4::pango::EllipsizeMode::Middle)
        .hexpand(true)
        .xalign(0.0)
        .build();
    let choose_btn = Button::builder().label("Choose image...").build();
    image_box.append(&path_label);
    image_box.append(&choose_btn);
    section.append(&image_box);

    let display = gtk4::prelude::WidgetExt::display(window);
    let theme = gtk4::IconTheme::for_display(&display);
    let mut names: Vec<String> = theme
        .icon_names()
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    names.sort_unstable();
    names.dedup();
    let all_icons: Rc<Vec<String>> = Rc::new(names);
    let symbolic_icons: Rc<Vec<String>> = Rc::new(
        all_icons
            .iter()
            .filter(|n| n.ends_with("-symbolic"))
            .cloned()
            .collect(),
    );

    let hibiki_preview = crate::ui::hibiki_logo_texture(24);

    let apply_mode = {
        let string_list = string_list.clone();
        let selection_model = selection_model.clone();
        let picker_box_c = picker_box.clone();
        let image_box_c = image_box.clone();
        let preview_c = preview.clone();
        let all = Rc::clone(&all_icons);
        let symbolic = Rc::clone(&symbolic_icons);
        let service_c = config_service.clone();

        move |mode: SuperKeyIconMode| {
            let is_picker = matches!(mode, SuperKeyIconMode::Symbolic | SuperKeyIconMode::Icon);
            picker_box_c.set_visible(is_picker);
            image_box_c.set_visible(mode == SuperKeyIconMode::Image);

            let list = icons_for_mode(mode, &all, &symbolic);
            let refs: Vec<&str> = list.iter().map(String::as_str).collect();
            string_list.splice(0, string_list.n_items(), &refs);

            let cfg = service_c.get_config();
            let icon_name = cfg.super_key_icon.icon_name.as_deref();
            let selected = icon_name
                .and_then(|name| list.iter().position(|n| n == name))
                .map_or(gtk4::INVALID_LIST_POSITION, |idx| idx as u32);
            selection_model.set_selected(selected);

            match mode {
                SuperKeyIconMode::Hibiki => {
                    if let Some(texture) = &hibiki_preview {
                        preview_c.set_paintable(Some(texture));
                    } else {
                        preview_c.clear();
                    }
                }
                SuperKeyIconMode::Symbolic | SuperKeyIconMode::Icon => {
                    match cfg.super_key_icon.icon_name.as_deref() {
                        Some(name) if !name.is_empty() => preview_c.set_icon_name(Some(name)),
                        _ => preview_c.clear(),
                    }
                }
                SuperKeyIconMode::Image => {
                    match cfg
                        .super_key_icon
                        .image_path
                        .as_deref()
                        .and_then(load_preview_texture)
                    {
                        Some(texture) => preview_c.set_paintable(Some(&texture)),
                        None => preview_c.clear(),
                    }
                }
            }
        }
    };

    apply_mode(current.mode);

    let filter_c = filter.clone();
    search_entry.connect_search_changed(move |entry| {
        let text = entry.text().to_lowercase();
        filter_c.set_filter_func(move |item| {
            item.downcast_ref::<gtk4::StringObject>()
                .is_some_and(|s| s.string().to_lowercase().contains(&text))
        });
    });

    let service_c = config_service.clone();
    let preview_c = preview.clone();
    selection_model.connect_selected_item_notify(move |model| {
        let Some(item) = model.selected_item() else {
            return;
        };
        let Ok(string_obj) = item.downcast::<gtk4::StringObject>() else {
            return;
        };
        let name = string_obj.string().to_string();

        let mut cfg = service_c.get_config();
        if !matches!(
            cfg.super_key_icon.mode,
            SuperKeyIconMode::Symbolic | SuperKeyIconMode::Icon
        ) || cfg.super_key_icon.icon_name.as_deref() == Some(name.as_str())
        {
            return;
        }

        cfg.super_key_icon.icon_name = Some(name.clone());
        let _ = service_c.update_config(cfg);
        preview_c.set_icon_name(Some(&name));
    });

    let service_c = config_service.clone();
    let apply_mode_c = apply_mode.clone();
    dropdown.connect_selected_notify(move |dd| {
        let mode = mode_from_index(dd.selected());
        let mut cfg = service_c.get_config();
        if cfg.super_key_icon.mode != mode {
            cfg.super_key_icon.mode = mode;
            let _ = service_c.update_config(cfg);
        }
        apply_mode_c(mode);
    });

    let service_c = config_service.clone();
    let path_label_c = path_label.clone();
    let preview_c = preview.clone();
    let window_c = window.clone();
    choose_btn.connect_clicked(move |_| {
        let filter = FileFilter::new();
        filter.set_name(Some("Images (PNG, WebP)"));
        filter.add_mime_type("image/png");
        filter.add_mime_type("image/webp");

        let dialog = FileDialog::builder()
            .title("Choose Super Key Image")
            .modal(true)
            .default_filter(&filter)
            .build();

        let service_c = service_c.clone();
        let path_label_c = path_label_c.clone();
        let preview_c = preview_c.clone();
        let window_c = window_c.clone();
        glib::MainContext::default().spawn_local(async move {
            if let Ok(file) = dialog.open_future(Some(&window_c)).await {
                let Some(path) = file.path() else {
                    return;
                };
                let path_str = path.to_string_lossy().to_string();

                let mut cfg = service_c.get_config();
                cfg.super_key_icon.mode = SuperKeyIconMode::Image;
                cfg.super_key_icon.image_path = Some(path_str.clone());
                let _ = service_c.update_config(cfg);

                path_label_c.set_label(&path_str);
                match load_preview_texture(&path_str) {
                    Some(texture) => preview_c.set_paintable(Some(&texture)),
                    None => preview_c.clear(),
                }
            }
        });
    });

    section
}

fn create_keystroke_settings(
    config_service: &ConfigService,
    typography_service: &TypographyService,
    window: &ApplicationWindow,
) -> GtkBox {
    let content_box = GtkBox::new(Orientation::Vertical, 0);
    content_box.set_margin_start(32);
    content_box.set_margin_end(32);
    content_box.set_valign(Align::Start);
    content_box.set_halign(Align::Fill);

    let header_box = GtkBox::new(Orientation::Horizontal, 12);
    header_box.set_margin_top(16);
    header_box.set_margin_bottom(16);
    let label = Label::builder()
        .label("General")
        .css_classes(vec!["title-2"])
        .build();
    header_box.append(&label);
    content_box.append(&header_box);

    let config = config_service.get_config();

    let general_card = GtkBox::new(Orientation::Vertical, 0);
    general_card.add_css_class("settings-card");
    general_card.set_margin_bottom(24);

    let grid = Grid::builder()
        .column_spacing(48)
        .row_spacing(24)
        .hexpand(true)
        .build();

    general_card.append(&grid);
    content_box.append(&general_card);

    let left_col = GtkBox::new(Orientation::Vertical, 24);
    left_col.set_hexpand(false);
    left_col.set_width_request(250);

    left_col.append(&create_theme_section(
        config.keystroke_theme.clone(),
        config_service,
    ));

    let service_c = config_service.clone();
    left_col.append(&create_slider_section(
        "OPACITY",
        config.opacity,
        0.0,
        1.0,
        0.01,
        SliderFormat::Percent,
        move |val| {
            let mut cfg = service_c.get_config();
            if (cfg.opacity - val).abs() > f64::EPSILON {
                cfg.opacity = val;
                let _ = service_c.update_config(cfg);
            }
        },
    ));

    let service_c = config_service.clone();
    left_col.append(&create_slider_section(
        "CORNER RADIUS",
        config.corner_radius,
        0.0,
        1.0,
        0.01,
        SliderFormat::Pixels,
        move |val| {
            let mut cfg = service_c.get_config();
            if (cfg.corner_radius - val).abs() > f64::EPSILON {
                cfg.corner_radius = val;
                let _ = service_c.update_config(cfg);
            }
        },
    ));

    left_col.append(&create_position_section(
        config.position,
        config_service,
        false,
    ));

    grid.attach(&left_col, 0, 0, 1, 1);

    let right_col = GtkBox::new(Orientation::Vertical, 32);
    right_col.set_hexpand(true);
    right_col.set_vexpand(true);
    right_col.set_width_request(350);

    create_font_selector(
        &right_col,
        typography_service,
        config_service,
        config.font_family.clone(),
        |cfg, name| cfg.font_family = name,
    );

    let service_c = config_service.clone();
    right_col.append(&create_slider_section(
        "SIZE",
        config.font_size,
        0.5,
        4.0,
        0.1,
        SliderFormat::Multiplier,
        move |val| {
            let mut cfg = service_c.get_config();
            if (cfg.font_size - val).abs() > f64::EPSILON {
                cfg.font_size = val;
                let _ = service_c.update_config(cfg);
            }
        },
    ));

    right_col.append(&create_color_section(config_service, false, window));

    grid.attach(&right_col, 1, 0, 1, 1);

    let super_key_label = Label::builder()
        .label("Super Key Icon")
        .css_classes(vec!["title-2"])
        .halign(Align::Start)
        .margin_bottom(16)
        .build();
    content_box.append(&super_key_label);

    let super_key_card = GtkBox::new(Orientation::Vertical, 0);
    super_key_card.add_css_class("settings-card");
    super_key_card.set_margin_bottom(24);
    super_key_card.append(&create_super_key_icon_section(config_service, window));
    content_box.append(&super_key_card);

    let behavior_label = Label::builder()
        .label("Behavior")
        .css_classes(vec!["title-2"])
        .halign(Align::Start)
        .margin_top(0)
        .margin_bottom(16)
        .build();
    content_box.append(&behavior_label);

    let behavior_card = GtkBox::new(Orientation::Vertical, 0);
    behavior_card.add_css_class("settings-card");
    behavior_card.set_margin_bottom(24);

    let behavior_grid = Grid::builder()
        .column_spacing(48)
        .row_spacing(24)
        .hexpand(true)
        .build();

    behavior_card.append(&behavior_grid);
    content_box.append(&behavior_card);

    let b_left = GtkBox::new(Orientation::Vertical, 32);
    b_left.set_hexpand(true);

    let dur_box = GtkBox::new(Orientation::Vertical, 12);
    let dur_header = GtkBox::new(Orientation::Horizontal, 0);
    let dur_label = Label::builder()
        .label("DISPLAY DURATION")
        .css_classes(vec!["heading", "caption"])
        .halign(Align::Start)
        .hexpand(true)
        .build();

    let dur_val_label = Label::builder()
        .label(format!("{:.1}s", config.display_timeout_ms as f64 / 1000.0))
        .css_classes(vec!["badge"])
        .build();

    dur_header.append(&dur_label);
    dur_header.append(&dur_val_label);
    dur_box.append(&dur_header);

    let dur_adj = Adjustment::new(
        config.display_timeout_ms as f64 / 1000.0,
        0.5,
        10.0,
        0.5,
        1.0,
        0.0,
    );
    let dur_scale = Scale::builder()
        .orientation(Orientation::Horizontal)
        .adjustment(&dur_adj)
        .draw_value(false)
        .build();
    dur_box.append(&dur_scale);
    b_left.append(&dur_box);

    let drag_box = GtkBox::new(Orientation::Horizontal, 12);
    let drag_text = GtkBox::new(Orientation::Vertical, 2);
    drag_text.set_hexpand(true);
    let drag_lbl = Label::builder()
        .label("Draggable Overlay")
        .xalign(0.0)
        .css_classes(vec!["body"])
        .build();
    let drag_sub = Label::builder()
        .label("Move visualizer with mouse")
        .xalign(0.0)
        .css_classes(vec!["caption", "dim-label"])
        .build();
    drag_text.append(&drag_lbl);
    drag_text.append(&drag_sub);
    drag_box.append(&drag_text);

    let drag_switch = Switch::builder()
        .active(config.keystroke_draggable)
        .valign(Align::Center)
        .build();
    drag_box.append(&drag_switch);
    b_left.append(&drag_box);

    let separate_box = GtkBox::new(Orientation::Horizontal, 12);
    let separate_text = GtkBox::new(Orientation::Vertical, 2);
    separate_text.set_hexpand(true);
    let separate_lbl = Label::builder()
        .label("Separate Repeated Keys")
        .xalign(0.0)
        .css_classes(vec!["body"])
        .build();
    let separate_sub = Label::builder()
        .label("Show every repeat in its own bubble")
        .xalign(0.0)
        .css_classes(vec!["caption", "dim-label"])
        .build();
    separate_text.append(&separate_lbl);
    separate_text.append(&separate_sub);
    separate_box.append(&separate_text);

    let separate_switch = Switch::builder()
        .active(config.separate_duplicate_keys)
        .valign(Align::Center)
        .build();
    separate_box.append(&separate_switch);
    b_left.append(&separate_box);

    let combine_box = GtkBox::new(Orientation::Horizontal, 12);
    let combine_text = GtkBox::new(Orientation::Vertical, 2);
    combine_text.set_hexpand(true);
    let combine_lbl = Label::builder()
        .label("Combine Held Modifier")
        .xalign(0.0)
        .css_classes(vec!["body"])
        .build();
    let combine_sub = Label::builder()
        .label("Keep modifier attached to following keys (ctrl + a + b)")
        .xalign(0.0)
        .css_classes(vec!["caption", "dim-label"])
        .build();
    combine_text.append(&combine_lbl);
    combine_text.append(&combine_sub);
    combine_box.append(&combine_text);

    let combine_switch = Switch::builder()
        .active(config.combine_modifier_keys)
        .valign(Align::Center)
        .build();
    combine_box.append(&combine_switch);
    b_left.append(&combine_box);

    let unify_box = GtkBox::new(Orientation::Horizontal, 12);
    let unify_text = GtkBox::new(Orientation::Vertical, 2);
    unify_text.set_hexpand(true);
    let unify_lbl = Label::builder()
        .label("Unify Words")
        .xalign(0.0)
        .css_classes(vec!["body"])
        .build();
    let unify_sub = Label::builder()
        .label("Group typed characters into one bubble per word")
        .xalign(0.0)
        .css_classes(vec!["caption", "dim-label"])
        .build();
    unify_text.append(&unify_lbl);
    unify_text.append(&unify_sub);
    unify_box.append(&unify_text);

    let unify_switch = Switch::builder()
        .active(config.unify_words)
        .valign(Align::Center)
        .build();
    unify_box.append(&unify_switch);
    b_left.append(&unify_box);

    behavior_grid.attach(&b_left, 0, 0, 1, 1);

    let b_right = GtkBox::new(Orientation::Vertical, 0);
    b_right.set_width_request(350);

    let card_content = GtkBox::new(Orientation::Vertical, 20);

    let hk_title = Label::builder()
        .label("GLOBAL HOTKEYS")
        .css_classes(vec!["heading", "caption"])
        .halign(Align::Start)
        .build();
    card_content.append(&hk_title);

    let hk1_box = GtkBox::new(Orientation::Horizontal, 12);
    hk1_box.add_css_class("settings-element");
    hk1_box.set_margin_bottom(8);
    let hk1_lbl = Label::builder()
        .label("Toggle Activation")
        .hexpand(true)
        .xalign(0.0)
        .build();
    let hk1_btn = Button::builder()
        .label(format_hotkey(&config.keystroke_hotkey))
        .valign(Align::Center)
        .build();
    hk1_box.append(&hk1_lbl);
    hk1_box.append(&hk1_btn);
    card_content.append(&hk1_box);

    let hk2_box = GtkBox::new(Orientation::Horizontal, 12);
    hk2_box.add_css_class("settings-element");
    hk2_box.set_margin_bottom(8);
    let hk2_lbl = Label::builder()
        .label("Pause/Resume")
        .hexpand(true)
        .xalign(0.0)
        .build();
    let hk2_btn = Button::builder()
        .label(format_hotkey(&config.pause_hotkey))
        .valign(Align::Center)
        .build();
    hk2_box.append(&hk2_lbl);
    hk2_box.append(&hk2_btn);
    card_content.append(&hk2_box);

    let hk3_box = GtkBox::new(Orientation::Horizontal, 12);
    hk3_box.add_css_class("settings-element");
    let hk3_lbl = Label::builder()
        .label("Toggle Mode")
        .hexpand(true)
        .xalign(0.0)
        .build();
    let hk3_btn = Button::builder()
        .label(format_hotkey(&config.toggle_mode_hotkey))
        .valign(Align::Center)
        .build();
    hk3_box.append(&hk3_lbl);
    hk3_box.append(&hk3_btn);
    card_content.append(&hk3_box);

    let backend_box = GtkBox::new(Orientation::Vertical, 6);
    backend_box.set_margin_top(12);
    let backend_lbl = Label::builder()
        .label("Shortcut Handling")
        .css_classes(vec!["caption", "dim-label"])
        .halign(Align::Start)
        .xalign(0.0)
        .build();
    let backend_dd = DropDown::from_strings(&["Automatic", "Desktop portal", "Direct (device)"]);
    backend_dd.set_halign(Align::Start);
    backend_dd.set_selected(backend_index(config.global_shortcuts));
    let backend_hint = Label::builder()
        .label(
            "The desktop portal lets the compositor own the shortcuts, so they no \
             longer reach the focused app. Takes effect after a restart.",
        )
        .css_classes(vec!["caption", "dim-label"])
        .wrap(true)
        .xalign(0.0)
        .build();
    backend_box.append(&backend_lbl);
    backend_box.append(&backend_dd);
    backend_box.append(&backend_hint);

    let backend_service = config_service.clone();
    backend_dd.connect_selected_notify(move |dd| {
        let mut cfg = backend_service.get_config();
        cfg.global_shortcuts = backend_from_index(dd.selected());
        let _ = backend_service.update_config(cfg);
    });

    b_left.append(&backend_box);

    b_right.append(&card_content);

    let combo_limit_box = GtkBox::new(Orientation::Horizontal, 12);
    combo_limit_box.set_valign(Align::Center);
    combo_limit_box.set_margin_top(24);
    let combo_limit_label = Label::builder()
        .label("Max Combined Keys")
        .css_classes(vec!["body"])
        .hexpand(true)
        .xalign(0.0)
        .build();
    combo_limit_box.append(&combo_limit_label);

    let combo_stepper = GtkBox::new(Orientation::Horizontal, 0);
    combo_stepper.add_css_class("linked");

    let combo_minus = Button::builder().label("-").width_request(32).build();
    let combo_display = Button::builder()
        .label(config.max_combo_keys.to_string())
        .width_request(40)
        .can_focus(false)
        .build();
    let combo_plus = Button::builder().label("+").width_request(32).build();

    combo_stepper.append(&combo_minus);
    combo_stepper.append(&combo_display);
    combo_stepper.append(&combo_plus);
    combo_limit_box.append(&combo_stepper);
    b_right.append(&combo_limit_box);

    let max_box = GtkBox::new(Orientation::Horizontal, 12);
    max_box.set_valign(Align::Center);
    max_box.set_margin_top(24);
    let max_label = Label::builder()
        .label("Max Visible Keys")
        .css_classes(vec!["body"])
        .hexpand(true)
        .xalign(0.0)
        .build();
    max_box.append(&max_label);

    let stepper_box = GtkBox::new(Orientation::Horizontal, 0);
    stepper_box.add_css_class("linked");

    let step_minus = Button::builder().label("-").width_request(32).build();
    let step_display = Button::builder()
        .label(config.max_keys.to_string())
        .width_request(40)
        .can_focus(false)
        .build();
    let step_plus = Button::builder().label("+").width_request(32).build();

    stepper_box.append(&step_minus);
    stepper_box.append(&step_display);
    stepper_box.append(&step_plus);
    max_box.append(&stepper_box);
    b_right.append(&max_box);

    let word_box = GtkBox::new(Orientation::Horizontal, 12);
    word_box.set_valign(Align::Center);
    word_box.set_margin_top(24);
    let word_label = Label::builder()
        .label("Max Word Length")
        .css_classes(vec!["body"])
        .hexpand(true)
        .xalign(0.0)
        .build();
    word_box.append(&word_label);

    let word_stepper = GtkBox::new(Orientation::Horizontal, 0);
    word_stepper.add_css_class("linked");

    let word_minus = Button::builder().label("-").width_request(32).build();
    let word_display = Button::builder()
        .label(config.max_word_length.to_string())
        .width_request(40)
        .can_focus(false)
        .build();
    let word_plus = Button::builder().label("+").width_request(32).build();

    word_stepper.append(&word_minus);
    word_stepper.append(&word_display);
    word_stepper.append(&word_plus);
    word_box.append(&word_stepper);
    b_right.append(&word_box);

    behavior_grid.attach(&b_right, 1, 0, 1, 1);

    let service_c = config_service.clone();
    let dur_val_c = dur_val_label.clone();
    dur_adj.connect_value_changed(move |adj| {
        let val = adj.value();
        dur_val_c.set_label(&format!("{:.1}s", val));
        let ms = (val * 1000.0) as u64;
        let mut cfg = service_c.get_config();
        if cfg.display_timeout_ms != ms {
            cfg.display_timeout_ms = ms;
            let _ = service_c.update_config(cfg);
        }
    });

    let service_minus = config_service.clone();
    let display_minus = step_display.clone();
    step_minus.connect_clicked(move |_| {
        let mut cfg = service_minus.get_config();
        if cfg.max_keys > 1 {
            cfg.max_keys -= 1;
            display_minus.set_label(&cfg.max_keys.to_string());
            let _ = service_minus.update_config(cfg);
        }
    });

    let service_plus = config_service.clone();
    let display_plus = step_display.clone();
    step_plus.connect_clicked(move |_| {
        let mut cfg = service_plus.get_config();
        if cfg.max_keys < 20 {
            cfg.max_keys += 1;
            display_plus.set_label(&cfg.max_keys.to_string());
            let _ = service_plus.update_config(cfg);
        }
    });

    let service_c = config_service.clone();
    drag_switch.connect_state_set(move |_, state| {
        let mut cfg = service_c.get_config();
        if cfg.keystroke_draggable != state {
            cfg.keystroke_draggable = state;
            let _ = service_c.update_config(cfg);
        }
        glib::Propagation::Proceed
    });

    let service_c = config_service.clone();
    separate_switch.connect_state_set(move |_, state| {
        let mut cfg = service_c.get_config();
        if cfg.separate_duplicate_keys != state {
            cfg.separate_duplicate_keys = state;
            let _ = service_c.update_config(cfg);
        }
        glib::Propagation::Proceed
    });

    let service_c = config_service.clone();
    combine_switch.connect_state_set(move |_, state| {
        let mut cfg = service_c.get_config();
        if cfg.combine_modifier_keys != state {
            cfg.combine_modifier_keys = state;
            let _ = service_c.update_config(cfg);
        }
        glib::Propagation::Proceed
    });

    let service_c = config_service.clone();
    unify_switch.connect_state_set(move |_, state| {
        let mut cfg = service_c.get_config();
        if cfg.unify_words != state {
            cfg.unify_words = state;
            let _ = service_c.update_config(cfg);
        }
        glib::Propagation::Proceed
    });

    let service_minus = config_service.clone();
    let combo_minus_display = combo_display.clone();
    combo_minus.connect_clicked(move |_| {
        let mut cfg = service_minus.get_config();
        if cfg.max_combo_keys > 2 {
            cfg.max_combo_keys -= 1;
            combo_minus_display.set_label(&cfg.max_combo_keys.to_string());
            let _ = service_minus.update_config(cfg);
        }
    });

    let service_plus = config_service.clone();
    let combo_plus_display = combo_display.clone();
    combo_plus.connect_clicked(move |_| {
        let mut cfg = service_plus.get_config();
        if cfg.max_combo_keys < 12 {
            cfg.max_combo_keys += 1;
            combo_plus_display.set_label(&cfg.max_combo_keys.to_string());
            let _ = service_plus.update_config(cfg);
        }
    });

    let service_minus = config_service.clone();
    let word_minus_display = word_display.clone();
    word_minus.connect_clicked(move |_| {
        let mut cfg = service_minus.get_config();
        if cfg.max_word_length > 1 {
            cfg.max_word_length -= 1;
            word_minus_display.set_label(&cfg.max_word_length.to_string());
            let _ = service_minus.update_config(cfg);
        }
    });

    let service_plus = config_service.clone();
    let word_plus_display = word_display.clone();
    word_plus.connect_clicked(move |_| {
        let mut cfg = service_plus.get_config();
        if cfg.max_word_length < 50 {
            cfg.max_word_length += 1;
            word_plus_display.set_label(&cfg.max_word_length.to_string());
            let _ = service_plus.update_config(cfg);
        }
    });

    setup_hotkey_capture(
        window,
        &hk1_btn,
        config_service.clone(),
        HotkeyType::KeystrokeActivation,
    );

    setup_hotkey_capture(window, &hk2_btn, config_service.clone(), HotkeyType::Pause);

    setup_hotkey_capture(
        window,
        &hk3_btn,
        config_service.clone(),
        HotkeyType::ToggleMode,
    );

    content_box
}

fn create_bubble_settings(
    config_service: &ConfigService,
    typography_service: &TypographyService,
    window: &ApplicationWindow,
) -> GtkBox {
    let content_box = GtkBox::new(Orientation::Vertical, 0);
    content_box.set_margin_start(32);
    content_box.set_margin_end(32);
    content_box.set_valign(Align::Start);
    content_box.set_halign(Align::Fill);

    let header_box = GtkBox::new(Orientation::Horizontal, 12);
    header_box.set_margin_top(16);
    header_box.set_margin_bottom(16);
    let label = Label::builder()
        .label("General")
        .css_classes(vec!["title-2"])
        .build();
    header_box.append(&label);
    content_box.append(&header_box);

    let config = config_service.get_config();

    let general_card = GtkBox::new(Orientation::Vertical, 0);
    general_card.add_css_class("settings-card");
    general_card.set_margin_bottom(24);

    let grid = Grid::builder()
        .column_spacing(48)
        .row_spacing(24)
        .hexpand(true)
        .build();

    general_card.append(&grid);
    content_box.append(&general_card);

    let left_col = GtkBox::new(Orientation::Vertical, 24);
    left_col.set_hexpand(false);
    left_col.set_width_request(250);

    left_col.append(&create_theme_section(
        config.keystroke_theme.clone(),
        config_service,
    ));

    let service_c = config_service.clone();
    left_col.append(&create_slider_section(
        "OPACITY",
        config.bubble.opacity,
        0.0,
        1.0,
        0.01,
        SliderFormat::Percent,
        move |val| {
            let mut cfg = service_c.get_config();
            if (cfg.bubble.opacity - val).abs() > f64::EPSILON {
                cfg.bubble.opacity = val;
                let _ = service_c.update_config(cfg);
            }
        },
    ));

    let service_c = config_service.clone();
    left_col.append(&create_slider_section(
        "CORNER RADIUS",
        config.bubble.corner_radius,
        0.0,
        1.0,
        0.01,
        SliderFormat::Pixels,
        move |val| {
            let mut cfg = service_c.get_config();
            if (cfg.bubble.corner_radius - val).abs() > f64::EPSILON {
                cfg.bubble.corner_radius = val;
                let _ = service_c.update_config(cfg);
            }
        },
    ));

    left_col.append(&create_position_section(
        config.bubble.position,
        config_service,
        true,
    ));

    grid.attach(&left_col, 0, 0, 1, 1);

    let right_col = GtkBox::new(Orientation::Vertical, 32);
    right_col.set_hexpand(true);
    right_col.set_vexpand(true);
    right_col.set_width_request(350);

    create_font_selector(
        &right_col,
        typography_service,
        config_service,
        config.bubble.font_family.clone(),
        |cfg, name| cfg.bubble.font_family = name,
    );

    let service_c = config_service.clone();
    right_col.append(&create_slider_section(
        "SIZE",
        config.bubble.font_size,
        0.5,
        4.0,
        0.1,
        SliderFormat::Multiplier,
        move |val| {
            let mut cfg = service_c.get_config();
            if (cfg.bubble.font_size - val).abs() > f64::EPSILON {
                cfg.bubble.font_size = val;
                let _ = service_c.update_config(cfg);
            }
        },
    ));

    right_col.append(&create_color_section(config_service, true, window));

    grid.attach(&right_col, 1, 0, 1, 1);

    let behavior_label = Label::builder()
        .label("Behavior")
        .css_classes(vec!["title-2"])
        .halign(Align::Start)
        .margin_top(0)
        .margin_bottom(16)
        .build();
    content_box.append(&behavior_label);

    let behavior_card = GtkBox::new(Orientation::Vertical, 0);
    behavior_card.add_css_class("settings-card");
    behavior_card.set_margin_bottom(24);

    let behavior_grid = Grid::builder()
        .column_spacing(48)
        .row_spacing(24)
        .hexpand(true)
        .build();

    behavior_card.append(&behavior_grid);
    content_box.append(&behavior_card);

    let b_left = GtkBox::new(Orientation::Vertical, 32);
    b_left.set_hexpand(true);

    let dur_box = GtkBox::new(Orientation::Vertical, 12);
    let dur_header = GtkBox::new(Orientation::Horizontal, 0);
    let dur_label = Label::builder()
        .label("DISPLAY DURATION")
        .css_classes(vec!["heading", "caption"])
        .halign(Align::Start)
        .hexpand(true)
        .build();

    let dur_val_label = Label::builder()
        .label(format!("{:.1}s", config.bubble.timeout_ms as f64 / 1000.0))
        .css_classes(vec!["badge"])
        .build();

    dur_header.append(&dur_label);
    dur_header.append(&dur_val_label);
    dur_box.append(&dur_header);

    let dur_adj = Adjustment::new(
        config.bubble.timeout_ms as f64 / 1000.0,
        0.5,
        10.0,
        0.5,
        1.0,
        0.0,
    );
    let dur_scale = Scale::builder()
        .orientation(Orientation::Horizontal)
        .adjustment(&dur_adj)
        .draw_value(false)
        .build();
    dur_box.append(&dur_scale);
    b_left.append(&dur_box);

    let drag_box = GtkBox::new(Orientation::Horizontal, 12);
    let drag_text = GtkBox::new(Orientation::Vertical, 2);
    drag_text.set_hexpand(true);
    let drag_lbl = Label::builder()
        .label("Draggable Overlay")
        .xalign(0.0)
        .css_classes(vec!["body"])
        .build();
    let drag_sub = Label::builder()
        .label("Move visualizer with mouse")
        .xalign(0.0)
        .css_classes(vec!["caption", "dim-label"])
        .build();
    drag_text.append(&drag_lbl);
    drag_text.append(&drag_sub);
    drag_box.append(&drag_text);

    let drag_switch = Switch::builder()
        .active(config.bubble.draggable)
        .valign(Align::Center)
        .build();
    drag_box.append(&drag_switch);
    b_left.append(&drag_box);

    behavior_grid.attach(&b_left, 0, 0, 1, 1);

    let b_right = GtkBox::new(Orientation::Vertical, 0);
    b_right.set_width_request(350);

    let card_content = GtkBox::new(Orientation::Vertical, 20);

    let hk_title = Label::builder()
        .label("GLOBAL HOTKEYS")
        .css_classes(vec!["heading", "caption"])
        .halign(Align::Start)
        .build();
    card_content.append(&hk_title);

    let hk1_box = GtkBox::new(Orientation::Horizontal, 12);
    hk1_box.add_css_class("settings-element");
    hk1_box.set_margin_bottom(8);
    let hk1_lbl = Label::builder()
        .label("Activation Hotkey")
        .hexpand(true)
        .xalign(0.0)
        .build();
    let hk1_btn = Button::builder()
        .label(format_hotkey(&config.bubble.hotkey))
        .valign(Align::Center)
        .build();
    hk1_box.append(&hk1_lbl);
    hk1_box.append(&hk1_btn);
    card_content.append(&hk1_box);

    let hk2_box = GtkBox::new(Orientation::Horizontal, 12);
    hk2_box.add_css_class("settings-element");
    let hk2_lbl = Label::builder()
        .label("Focus Hotkey")
        .hexpand(true)
        .xalign(0.0)
        .build();
    let hk2_btn = Button::builder()
        .label(format_hotkey(&config.toggle_focus_hotkey))
        .valign(Align::Center)
        .build();
    hk2_box.append(&hk2_lbl);
    hk2_box.append(&hk2_btn);
    card_content.append(&hk2_box);

    b_right.append(&card_content);
    behavior_grid.attach(&b_right, 1, 0, 1, 1);

    let service_c = config_service.clone();
    let dur_val_c = dur_val_label.clone();
    dur_adj.connect_value_changed(move |adj| {
        let val = adj.value();
        dur_val_c.set_label(&format!("{:.1}s", val));

        let ms = (val * 1000.0) as u64;
        let mut cfg = service_c.get_config();
        if cfg.bubble.timeout_ms != ms {
            cfg.bubble.timeout_ms = ms;
            let _ = service_c.update_config(cfg);
        }
    });

    let service_c = config_service.clone();
    drag_switch.connect_state_set(move |_, state| {
        let mut cfg = service_c.get_config();
        if cfg.bubble.draggable != state {
            cfg.bubble.draggable = state;
            let _ = service_c.update_config(cfg);
        }
        glib::Propagation::Proceed
    });

    setup_hotkey_capture(
        window,
        &hk1_btn,
        config_service.clone(),
        HotkeyType::BubbleActivation,
    );

    setup_hotkey_capture(
        window,
        &hk2_btn,
        config_service.clone(),
        HotkeyType::ToggleFocus,
    );

    content_box
}

pub fn show_settings(window: &ApplicationWindow) {
    window.present();
}

enum HotkeyType {
    KeystrokeActivation,
    BubbleActivation,
    Pause,
    ToggleFocus,
    ToggleMode,
}

fn setup_hotkey_capture(
    window: &ApplicationWindow,
    button: &Button,
    config_service: ConfigService,
    hotkey_type: HotkeyType,
) {
    let controller = EventControllerKey::new();
    let listening = Rc::new(RefCell::new(false));

    let listening_c = listening.clone();
    let btn_c = button.clone();
    let service_c = config_service.clone();

    controller.connect_key_pressed(move |_, keyval, _keycode, state| {
        if *listening_c.borrow() {
            if is_modifier(keyval) {
                let accelerator = gtk4::accelerator_name(keyval, state);
                btn_c.set_label(&format_hotkey(&accelerator));
                return glib::Propagation::Stop;
            }

            let accelerator = gtk4::accelerator_name(keyval, state);
            if !accelerator.is_empty() {
                let mut cfg = service_c.get_config();
                match hotkey_type {
                    HotkeyType::KeystrokeActivation => {
                        cfg.keystroke_hotkey = accelerator.to_string()
                    }
                    HotkeyType::BubbleActivation => cfg.bubble.hotkey = accelerator.to_string(),
                    HotkeyType::Pause => cfg.pause_hotkey = accelerator.to_string(),
                    HotkeyType::ToggleFocus => cfg.toggle_focus_hotkey = accelerator.to_string(),
                    HotkeyType::ToggleMode => cfg.toggle_mode_hotkey = accelerator.to_string(),
                }

                let _ = service_c.update_config(cfg);

                btn_c.set_label(&format_hotkey(&accelerator));
                btn_c.remove_css_class("suggested-action");
                *listening_c.borrow_mut() = false;
                return glib::Propagation::Stop;
            }
        }
        glib::Propagation::Proceed
    });

    window.add_controller(controller);

    let listening_c = listening.clone();
    button.connect_clicked(move |btn| {
        *listening_c.borrow_mut() = true;
        btn.set_label("Press keys...");
        btn.add_css_class("suggested-action");
    });
}

fn reload_pack_list(
    string_list: &StringList,
    display_names: &Rc<RefCell<HashMap<String, String>>>,
    selection_model: &SingleSelection,
    pack_list: &ListView,
    selected_pack: &str,
    extra_dir: Option<&str>,
) {
    let packs = SoundPackLoader::list_available_packs(extra_dir);

    {
        let mut map = display_names.borrow_mut();
        map.clear();
        for (id, name) in &packs {
            map.insert(id.clone(), name.clone());
        }
    }

    let ids: Vec<&str> = packs.iter().map(|(id, _)| id.as_str()).collect();
    string_list.splice(0, string_list.n_items(), &ids);

    if let Some(idx) = packs.iter().position(|(id, _)| id == selected_pack) {
        selection_model.set_selected(idx as u32);
        let pack_list = pack_list.clone();
        glib::timeout_add_local_once(std::time::Duration::from_millis(50), move || {
            pack_list.scroll_to(
                idx as u32,
                ListScrollFlags::FOCUS | ListScrollFlags::SELECT,
                None,
            );
        });
    }
}

fn create_sound_settings(config_service: &ConfigService, window: &ApplicationWindow) -> GtkBox {
    SoundPackLoader::ensure_user_sounds_dir();

    let content_box = GtkBox::new(Orientation::Vertical, 0);
    content_box.set_margin_start(32);
    content_box.set_margin_end(32);
    content_box.set_valign(Align::Start);
    content_box.set_halign(Align::Fill);

    let header_box = GtkBox::new(Orientation::Horizontal, 12);
    header_box.set_margin_top(16);
    header_box.set_margin_bottom(16);
    let header = Label::builder()
        .label("Sounds")
        .css_classes(vec!["title-2"])
        .build();
    header_box.append(&header);
    content_box.append(&header_box);

    let config = config_service.get_config();

    let general_card = GtkBox::new(Orientation::Vertical, 0);
    general_card.add_css_class("settings-card");
    general_card.set_margin_bottom(24);
    general_card.set_spacing(20);

    let enable_box = GtkBox::new(Orientation::Horizontal, 12);
    let enable_text = GtkBox::new(Orientation::Vertical, 2);
    enable_text.set_hexpand(true);
    let enable_lbl = Label::builder()
        .label("Enable Sounds")
        .xalign(0.0)
        .css_classes(vec!["body"])
        .build();
    let enable_sub = Label::builder()
        .label("Play sounds for every key press.")
        .xalign(0.0)
        .css_classes(vec!["caption", "dim-label"])
        .build();
    enable_text.append(&enable_lbl);
    enable_text.append(&enable_sub);
    enable_box.append(&enable_text);

    let enabled_switch = Switch::builder()
        .active(config.sound.enabled)
        .valign(Align::Center)
        .build();
    enable_box.append(&enabled_switch);
    general_card.append(&enable_box);

    let visual_box = GtkBox::new(Orientation::Horizontal, 12);
    let visual_text = GtkBox::new(Orientation::Vertical, 2);
    visual_text.set_hexpand(true);
    let visual_lbl = Label::builder()
        .label("Show Visualizer")
        .xalign(0.0)
        .css_classes(vec!["body"])
        .build();
    let visual_sub = Label::builder()
        .label("Draw keystrokes or bubbles on screen. Turn off to only hear sounds.")
        .xalign(0.0)
        .css_classes(vec!["caption", "dim-label"])
        .build();
    visual_text.append(&visual_lbl);
    visual_text.append(&visual_sub);
    visual_box.append(&visual_text);

    let visual_switch = Switch::builder()
        .active(config.show_visual)
        .valign(Align::Center)
        .build();
    visual_box.append(&visual_switch);
    general_card.append(&visual_box);

    visual_switch.set_sensitive(config.sound.enabled);

    content_box.append(&general_card);

    let visual_switch_c = visual_switch.clone();
    let service_c = config_service.clone();
    enabled_switch.connect_state_set(move |_, state| {
        let mut cfg = service_c.get_config();
        if cfg.sound.enabled != state {
            cfg.sound.enabled = state;
            let _ = service_c.update_config(cfg);
        }
        visual_switch_c.set_sensitive(state);
        glib::Propagation::Proceed
    });

    let service_c = config_service.clone();
    visual_switch.connect_state_set(move |_, state| {
        let mut cfg = service_c.get_config();
        if cfg.show_visual != state {
            cfg.show_visual = state;
            let _ = service_c.update_config(cfg);
        }
        glib::Propagation::Proceed
    });

    let section_card = GtkBox::new(Orientation::Vertical, 0);
    section_card.add_css_class("settings-card");
    section_card.set_margin_bottom(24);

    let grid = Grid::builder()
        .column_spacing(24)
        .row_spacing(24)
        .hexpand(true)
        .build();

    let left_col = GtkBox::new(Orientation::Vertical, 12);
    left_col.set_hexpand(true);

    let pack_label = Label::builder()
        .label("SOUND PACK")
        .css_classes(vec!["heading", "caption"])
        .halign(Align::Start)
        .build();
    left_col.append(&pack_label);

    let folder_row = GtkBox::new(Orientation::Horizontal, 8);
    let folder_value = Label::builder()
        .label(
            config
                .sound
                .folder
                .clone()
                .unwrap_or_else(|| "No extra folder".to_string()),
        )
        .css_classes(vec!["caption", "dim-label"])
        .ellipsize(gtk4::pango::EllipsizeMode::Middle)
        .hexpand(true)
        .xalign(0.0)
        .build();
    let folder_btn = Button::builder()
        .label("Choose folder…")
        .valign(Align::Center)
        .build();
    let info_btn = Button::builder()
        .icon_name("dialog-information-symbolic")
        .tooltip_text("How to add your own sound packs")
        .valign(Align::Center)
        .build();
    folder_row.append(&folder_value);
    folder_row.append(&folder_btn);
    folder_row.append(&info_btn);
    left_col.append(&folder_row);

    let search_entry = SearchEntry::builder()
        .placeholder_text("Find sound...")
        .build();
    left_col.append(&search_entry);

    let pack_scroll = ScrolledWindow::builder()
        .min_content_height(120)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .vexpand(false)
        .css_classes(vec!["font-list-frame"])
        .overflow(Overflow::Hidden)
        .build();

    let pack_list = ListView::new(None::<SelectionModel>, None::<ListItemFactory>);
    pack_list.add_css_class("navigation-sidebar");

    let string_list = StringList::new(&[]);
    let filter = CustomFilter::new(move |_item| true);
    let filter_model = FilterListModel::new(Some(string_list.clone()), Some(filter.clone()));
    let selection_model = SingleSelection::new(Some(filter_model.clone()));

    let factory = SignalListItemFactory::new();
    let display_names = Rc::new(RefCell::new(HashMap::<String, String>::new()));

    factory.connect_setup(move |_, list_item| {
        let list_item = list_item
            .downcast_ref::<gtk4::ListItem>()
            .expect("list_item must be a gtk4::ListItem");
        let row_box = GtkBox::new(Orientation::Horizontal, 12);
        row_box.set_margin_start(12);
        row_box.set_margin_end(12);
        row_box.set_margin_top(8);
        row_box.set_margin_bottom(8);

        let label = Label::builder().xalign(0.0).hexpand(true).build();

        row_box.append(&label);

        list_item.set_child(Some(&row_box));
    });

    let display_names_c = display_names.clone();
    factory.connect_bind(move |_, list_item| {
        let list_item = list_item
            .downcast_ref::<gtk4::ListItem>()
            .expect("list_item must be a gtk4::ListItem");
        let string_obj = list_item
            .item()
            .and_downcast::<gtk4::StringObject>()
            .expect("item must be a gtk4::StringObject");
        let pack_id = string_obj.string();

        let row_box = list_item
            .child()
            .and_downcast::<GtkBox>()
            .expect("child must be a GtkBox");
        let label = row_box
            .first_child()
            .and_downcast::<Label>()
            .expect("first_child must be a Label");

        let name = if let Ok(map) = display_names_c.try_borrow() {
            let map: &HashMap<String, String> = &map;
            map.get(pack_id.as_str())
                .cloned()
                .unwrap_or_else(|| pack_id.to_string())
        } else {
            pack_id.to_string()
        };

        label.set_label(&name);
    });

    pack_list.set_model(Some(&selection_model));
    pack_list.set_factory(Some(&factory));

    pack_list.connect_map(move |pack_list| {
        if let Some(model) = pack_list.model() {
            if let Some(selection_model) = model.downcast_ref::<SingleSelection>() {
                let selected = selection_model.selected();
                if selected != gtk4::INVALID_LIST_POSITION {
                    pack_list.scroll_to(
                        selected,
                        ListScrollFlags::FOCUS | ListScrollFlags::SELECT,
                        None,
                    );

                    let pack_list_weak = pack_list.downgrade();
                    glib::timeout_add_local_once(
                        std::time::Duration::from_millis(100),
                        move || {
                            if let Some(pack_list) = pack_list_weak.upgrade() {
                                pack_list.scroll_to(
                                    selected,
                                    ListScrollFlags::FOCUS | ListScrollFlags::SELECT,
                                    None,
                                );
                            }
                        },
                    );
                }
            }
        }
    });

    reload_pack_list(
        &string_list,
        &display_names,
        &selection_model,
        &pack_list,
        &config.sound.sound_pack,
        config.sound.folder.as_deref(),
    );

    let window_c = window.clone();
    let service_c = config_service.clone();
    let string_list_c = string_list.clone();
    let display_names_c = display_names.clone();
    let selection_c = selection_model.clone();
    let pack_list_c = pack_list.clone();
    let folder_value_c = folder_value.clone();
    folder_btn.connect_clicked(move |_| {
        let dialog = FileDialog::builder()
            .title("Choose sounds folder")
            .modal(true)
            .build();
        let window_c = window_c.clone();
        let service_c = service_c.clone();
        let string_list_c = string_list_c.clone();
        let display_names_c = display_names_c.clone();
        let selection_c = selection_c.clone();
        let pack_list_c = pack_list_c.clone();
        let folder_value_c = folder_value_c.clone();
        glib::MainContext::default().spawn_local(async move {
            if let Ok(folder) = dialog.select_folder_future(Some(&window_c)).await {
                let Some(path) = folder.path() else {
                    return;
                };
                let path_str = path.to_string_lossy().to_string();

                let mut cfg = service_c.get_config();
                cfg.sound.folder = Some(path_str.clone());
                let pack = cfg.sound.sound_pack.clone();
                let _ = service_c.update_config(cfg);

                folder_value_c.set_label(&path_str);
                reload_pack_list(
                    &string_list_c,
                    &display_names_c,
                    &selection_c,
                    &pack_list_c,
                    &pack,
                    Some(path_str.as_str()),
                );
            }
        });
    });

    let window_c = window.clone();
    info_btn.connect_clicked(move |_| {
        let dialog = gtk4::AlertDialog::builder()
            .modal(true)
            .message("Adding your own sounds")
            .detail(
                "A sound pack is a folder containing a config.json (Mechvibes format) plus its \
                 audio files (WAV or OGG).\n\nSingle packs reference one file in \"sound\"; \
                 multi packs reference one file per key in \"defines\".\n\nPacks placed in \
                 ~/.local/share/hibiki/sounds are always loaded. Choose any folder here to load \
                 packs from it as well.",
            )
            .build();
        dialog.show(Some(&window_c));
    });

    let filter_c = filter.clone();
    search_entry.connect_search_changed(move |entry| {
        let text = entry.text().to_lowercase();
        filter_c.set_filter_func(move |item| {
            let string_obj = item
                .downcast_ref::<gtk4::StringObject>()
                .expect("item must be a gtk4::StringObject");
            let pack_id = string_obj.string().to_lowercase();
            pack_id.contains(&text)
        });
    });

    let service_c = config_service.clone();
    selection_model.connect_selected_item_notify(move |model| {
        if let Some(item) = model.selected_item() {
            let string_obj = item
                .downcast::<gtk4::StringObject>()
                .expect("item must be a gtk4::StringObject");
            let id = string_obj.string().to_string();

            let mut cfg = service_c.get_config();
            if cfg.sound.sound_pack != id {
                cfg.sound.sound_pack = id;
                let _ = service_c.update_config(cfg);
            }
        }
    });

    pack_scroll.set_child(Some(&pack_list));
    left_col.append(&pack_scroll);
    grid.attach(&left_col, 0, 0, 1, 1);

    let right_col = GtkBox::new(Orientation::Vertical, 0);
    right_col.set_width_request(350);
    right_col.set_valign(Align::End);

    let vol_card = GtkBox::new(Orientation::Vertical, 16);
    vol_card.add_css_class("settings-element");

    let vol_header = GtkBox::new(Orientation::Horizontal, 0);
    let vol_label = Label::builder()
        .label("VOLUME")
        .css_classes(vec!["heading", "caption"])
        .halign(Align::Start)
        .hexpand(true)
        .build();

    let vol_val_label = Label::builder()
        .label(format!("{:.0}%", config.sound.volume * 100.0))
        .css_classes(vec!["badge"])
        .build();

    vol_header.append(&vol_label);
    vol_header.append(&vol_val_label);
    vol_card.append(&vol_header);

    let vol_slider_box = GtkBox::new(Orientation::Horizontal, 12);
    let vol_icon_low = gtk4::Image::from_icon_name("audio-volume-low-symbolic");
    let vol_icon_high = gtk4::Image::from_icon_name("audio-volume-high-symbolic");

    let vol_adj = Adjustment::new(
        config.sound.volume as f64 * 100.0,
        0.0,
        100.0,
        1.0,
        10.0,
        0.0,
    );
    let vol_scale = Scale::builder()
        .orientation(Orientation::Horizontal)
        .adjustment(&vol_adj)
        .draw_value(false)
        .hexpand(true)
        .build();

    vol_slider_box.append(&vol_icon_low);
    vol_slider_box.append(&vol_scale);
    vol_slider_box.append(&vol_icon_high);
    vol_card.append(&vol_slider_box);

    let service_c = config_service.clone();
    let vol_val_c = vol_val_label.clone();
    vol_adj.connect_value_changed(move |adj| {
        let val = adj.value();
        vol_val_c.set_label(&format!("{:.0}%", val));
        let volume = (val / 100.0) as f32;
        let mut cfg = service_c.get_config();
        if (cfg.sound.volume - volume).abs() > f32::EPSILON {
            cfg.sound.volume = volume;
            let _ = service_c.update_config(cfg);
        }
    });

    right_col.append(&vol_card);
    grid.attach(&right_col, 1, 0, 1, 1);

    section_card.append(&grid);
    content_box.append(&section_card);

    content_box
}

pub fn format_hotkey(accelerator: &str) -> String {
    if accelerator.is_empty() {
        return "None".to_string();
    }
    if let Some((key, mods)) = gtk4::accelerator_parse(accelerator) {
        let label = gtk4::accelerator_get_label(key, mods);
        label.replace("+", " + ")
    } else {
        accelerator.to_string()
    }
}

fn is_modifier(key: gtk4::gdk::Key) -> bool {
    use gtk4::gdk::Key;
    matches!(
        key,
        Key::Control_L
            | Key::Control_R
            | Key::Alt_L
            | Key::Alt_R
            | Key::Shift_L
            | Key::Shift_R
            | Key::Super_L
            | Key::Super_R
            | Key::Meta_L
            | Key::Meta_R
            | Key::Caps_Lock
            | Key::Shift_Lock
    )
}
