use crate::input::{is_modifier, is_super_key, KeyDisplay};
use evdev::Key;
use gtk4::gdk::Texture;
use gtk4::gdk_pixbuf::Pixbuf;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Image, Label, Orientation, Widget};
use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant};

const LOGO_SVG: &[u8] = include_bytes!("../assets/logo-symbolic.svg");

#[derive(Debug)]
struct DisplayedKey {
    keys: Vec<Key>,

    last_active: Instant,

    is_held: bool,

    widget: Widget,
}

pub struct KeyDisplayWidget {
    container: GtkBox,

    displayed_keys: VecDeque<DisplayedKey>,

    held_modifiers: HashSet<Key>,

    held_keys: HashSet<Key>,

    max_keys: usize,

    display_duration: Duration,

    logo_texture: Option<Texture>,
}

impl KeyDisplayWidget {
    pub fn new(max_keys: usize, display_timeout_ms: u64) -> Self {
        let container = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .halign(gtk4::Align::Start)
            .valign(gtk4::Align::Center)
            .build();

        container.add_css_class("keystroke-container");
        container.set_margin_top(25);

        let logo_texture = Self::load_logo_texture();

        Self {
            container,
            displayed_keys: VecDeque::new(),
            held_modifiers: HashSet::new(),
            held_keys: HashSet::new(),
            max_keys,
            display_duration: Duration::from_millis(display_timeout_ms),
            logo_texture,
        }
    }

    fn load_logo_texture() -> Option<Texture> {
        let (bg_color, fg_color) = {
            let dummy = gtk4::Label::new(None);
            #[allow(deprecated)]
            let style_context = dummy.style_context();

            let to_hex = |rgba: gtk4::gdk::RGBA| {
                format!(
                    "#{:02x}{:02x}{:02x}",
                    (rgba.red() * 255.0) as u8,
                    (rgba.green() * 255.0) as u8,
                    (rgba.blue() * 255.0) as u8
                )
            };

            #[allow(deprecated)]
            let fg = style_context
                .lookup_color("accent_fg_color")
                .unwrap_or(gtk4::gdk::RGBA::WHITE);

            #[allow(deprecated)]
            let bg = style_context
                .lookup_color("accent_bg_color")
                .or_else(|| style_context.lookup_color("theme_selected_bg_color"))
                .unwrap_or(gtk4::gdk::RGBA::new(0.18, 0.76, 0.49, 1.0));

            (to_hex(bg), to_hex(fg))
        };

        let svg_str = String::from_utf8_lossy(LOGO_SVG);
        let svg_with_color = svg_str
            .replace("currentColor", &fg_color)
            .replace("currentBackgroundColor", &bg_color);

        let stream = gtk4::gio::MemoryInputStream::from_bytes(&gtk4::glib::Bytes::from(
            svg_with_color.as_bytes(),
        ));

        if let Ok(pixbuf) =
            Pixbuf::from_stream_at_scale(&stream, 18, 18, false, gtk4::gio::Cancellable::NONE)
        {
            Some(Texture::for_pixbuf(&pixbuf))
        } else {
            None
        }
    }

    pub fn widget(&self) -> &GtkBox {
        &self.container
    }

    pub fn add_key(&mut self, key: KeyDisplay) {
        self.held_keys.insert(key.key);

        if is_modifier(key.key) {
            self.held_modifiers.insert(key.key);

            // Show modifiers when held alone
            self.update_modifier_display();
            return;
        }

        // Remove any modifier-only display since we're now showing a combo
        self.remove_modifier_only_display();

        let current_combo = self.get_current_combo(key.key);
        if let Some(existing) = self
            .displayed_keys
            .iter_mut()
            .find(|dk| dk.keys == current_combo)
        {
            existing.last_active = Instant::now();
            existing.is_held = true;
            existing.widget.remove_css_class("fading");
            return;
        }

        self.remove_expired();

        while self.displayed_keys.len() >= self.max_keys {
            if let Some(old) = self.displayed_keys.pop_front() {
                self.container.remove(&old.widget);
            }
        }

        let widget = self.create_combo_widget(&current_combo);

        self.container.append(&widget);

        let displayed = DisplayedKey {
            keys: current_combo,
            last_active: Instant::now(),
            is_held: true,
            widget,
        };

        self.displayed_keys.push_back(displayed);
    }

    fn get_current_combo(&self, key: Key) -> Vec<Key> {
        let mut combo: Vec<Key> = self.held_modifiers.iter().copied().collect();

        combo.sort_by_key(|k| match *k {
            Key::KEY_LEFTCTRL | Key::KEY_RIGHTCTRL => 0,
            Key::KEY_LEFTALT | Key::KEY_RIGHTALT => 1,
            Key::KEY_LEFTSHIFT | Key::KEY_RIGHTSHIFT => 2,
            Key::KEY_LEFTMETA | Key::KEY_RIGHTMETA => 3,
            _ => 4,
        });
        combo.push(key);
        combo
    }

    fn create_combo_widget(&self, keys: &[Key]) -> Widget {
        let key_box = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .hexpand(false)
            .build();
        key_box.add_css_class("keystroke-key");

        let has_modifier = keys.iter().any(|k| is_modifier(*k));
        if has_modifier {
            key_box.add_css_class("modifier");
        }

        for (i, key) in keys.iter().enumerate() {
            if i > 0 {
                let sep = Label::new(Some("+"));
                sep.add_css_class("combo-separator");
                sep.set_halign(gtk4::Align::Center);
                key_box.append(&sep);
            }

            if is_super_key(*key) {
                let image = self.create_logo_image();
                image.set_halign(gtk4::Align::Center);
                image.set_valign(gtk4::Align::Center);
                key_box.append(&image);
            } else {
                let display_name = crate::input::keymap::key_to_display_name(*key);
                let label = Label::builder()
                    .label(&display_name)
                    .halign(gtk4::Align::Center)
                    .valign(gtk4::Align::Center)
                    .hexpand(true)
                    .build();
                key_box.append(&label);
            }
        }

        key_box.upcast()
    }

    fn create_logo_image(&self) -> Image {
        if let Some(texture) = &self.logo_texture {
            let image = Image::from_paintable(Some(texture));
            image.set_pixel_size(18);
            image
        } else {
            Image::new()
        }
    }

    pub fn remove_key(&mut self, key: &KeyDisplay) {
        self.held_keys.remove(&key.key);

        if is_modifier(key.key) {
            self.held_modifiers.remove(&key.key);
            // Update modifier display when a modifier is released
            self.update_modifier_display();
        }

        for displayed in self.displayed_keys.iter_mut() {
            if displayed.keys.contains(&key.key) {
                displayed.is_held = false;
                displayed.last_active = Instant::now();
                displayed.widget.add_css_class("fading");
            }
        }
    }

    pub fn remove_expired(&mut self) {
        let now = Instant::now();
        let display_duration = self.display_duration;

        let mut to_remove = Vec::new();
        self.displayed_keys.retain(|dk| {
            let is_expired =
                !dk.is_held && now.saturating_duration_since(dk.last_active) > display_duration;
            if is_expired {
                to_remove.push(dk.widget.clone());
                false
            } else {
                true
            }
        });

        for widget in to_remove {
            self.container.remove(&widget);
        }
    }

    pub fn clear(&mut self) {
        while let Some(child) = self.container.first_child() {
            self.container.remove(&child);
        }
        self.displayed_keys.clear();
        self.held_keys.clear();
        self.held_modifiers.clear();
    }

    #[allow(dead_code)]
    pub fn set_display_timeout(&mut self, timeout_ms: u64) {
        self.display_duration = Duration::from_millis(timeout_ms);
    }

    #[allow(dead_code)]
    pub fn set_max_keys(&mut self, max_keys: usize) {
        self.max_keys = max_keys;

        while self.displayed_keys.len() > max_keys {
            if let Some(old) = self.displayed_keys.pop_front() {
                self.container.remove(&old.widget);
            }
        }
    }

    #[allow(dead_code)]
    pub fn has_keys(&self) -> bool {
        !self.displayed_keys.is_empty()
    }

    fn update_modifier_display(&mut self) {
        // Remove existing modifier-only display
        self.remove_modifier_only_display();

        // If we have held modifiers and no non-modifier keys are held, show them
        if !self.held_modifiers.is_empty() && self.held_keys.iter().all(|k| is_modifier(*k)) {
            let mut modifier_combo: Vec<Key> = self.held_modifiers.iter().copied().collect();
            modifier_combo.sort_by_key(|k| match *k {
                Key::KEY_LEFTCTRL | Key::KEY_RIGHTCTRL => 0,
                Key::KEY_LEFTALT | Key::KEY_RIGHTALT => 1,
                Key::KEY_LEFTSHIFT | Key::KEY_RIGHTSHIFT => 2,
                Key::KEY_LEFTMETA | Key::KEY_RIGHTMETA => 3,
                _ => 4,
            });

            let widget = self.create_combo_widget(&modifier_combo);
            self.container.append(&widget);

            let displayed = DisplayedKey {
                keys: modifier_combo,
                last_active: Instant::now(),
                is_held: true,
                widget,
            };

            self.displayed_keys.push_back(displayed);
        }
    }

    fn remove_modifier_only_display(&mut self) {
        // Remove any display entry that contains only modifiers
        let modifier_only_indices: Vec<usize> = self
            .displayed_keys
            .iter()
            .enumerate()
            .filter(|(_, dk)| dk.keys.iter().all(|k| is_modifier(*k)))
            .map(|(i, _)| i)
            .collect();

        for &i in modifier_only_indices.iter().rev() {
            if let Some(removed) = self.displayed_keys.remove(i) {
                self.container.remove(&removed.widget);
            }
        }
    }
}
