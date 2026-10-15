use crate::input::{is_layout_key, is_modifier, is_super_key, KeyDisplay, XkbState};
use evdev::Key;
use gtk4::gdk::Texture;
use gtk4::gdk_pixbuf::Pixbuf;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Image, Label, Orientation, Widget};
use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant};

const LOGO_SVG: &[u8] = include_bytes!("../assets/logo-symbolic.svg");

/// Label shown for a pressed key. Layout-dependent keys (letters, digits,
/// punctuation) are translated through `xkb_state`, so the visualizer matches
/// the active layout in both display modes; everything else keeps its symbolic
/// name.
fn key_label_from_layout(key: Key, xkb_state: &XkbState) -> String {
    if is_layout_key(key) {
        if let Some(utf8) = xkb_state.key_get_utf8(key) {
            let mut chars = utf8.chars();
            if let (Some(c), None) = (chars.next(), chars.next()) {
                if !c.is_control() {
                    return c.to_string();
                }
            }
        }
    }

    crate::input::keymap::key_to_display_name(key)
}

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

    xkb_state: XkbState,
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

        let logo_texture = Self::load_logo_texture();

        Self {
            container,
            displayed_keys: VecDeque::new(),
            held_modifiers: HashSet::new(),
            held_keys: HashSet::new(),
            max_keys,
            display_duration: Duration::from_millis(display_timeout_ms),
            logo_texture,
            xkb_state: XkbState::default(),
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

    /// Apply a keyboard layout so character keys render what the layout produces.
    pub fn set_layout(&mut self, layout_name: &str) {
        self.xkb_state.set_layout(layout_name);
    }

    /// Resolve the label for a key through the active layout, falling back to the
    /// symbolic name for keys that do not produce a layout character.
    fn key_label(&self, key: Key) -> String {
        key_label_from_layout(key, &self.xkb_state)
    }

    pub fn add_key(&mut self, key: KeyDisplay) {
        self.held_keys.insert(key.key);

        if is_modifier(key.key) {
            self.held_modifiers.insert(key.key);
            if !key.is_repeat {
                self.xkb_state.update_key(key.key, true);
            }

            return;
        }

        if !key.is_repeat {
            self.xkb_state.update_key(key.key, true);
        }

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
                let display_name = self.key_label(*key);
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
        }

        self.xkb_state.update_key(key.key, false);

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_label_uses_layout_for_letters() {
        let state = XkbState::from_layout_name(Some("English (US)")).unwrap();
        assert_eq!(key_label_from_layout(Key::KEY_A, &state), "a");
        assert_eq!(key_label_from_layout(Key::KEY_1, &state), "1");
    }

    #[test]
    fn test_key_label_respects_layout_remap() {
        // On QWERTZ (German) the physical Y/Z keys are swapped.
        let state = XkbState::from_layout_name(Some("German")).unwrap();
        assert_eq!(key_label_from_layout(Key::KEY_Y, &state), "z");
        assert_eq!(key_label_from_layout(Key::KEY_Z, &state), "y");
    }

    #[test]
    fn test_key_label_keeps_symbolic_keys() {
        let state = XkbState::from_layout_name(Some("English (US)")).unwrap();
        assert_eq!(key_label_from_layout(Key::KEY_LEFTCTRL, &state), "󰘴");
        assert_eq!(key_label_from_layout(Key::KEY_F1, &state), "F1");
        assert_eq!(key_label_from_layout(Key::KEY_SPACE, &state), "󱁐");
    }

    #[test]
    fn test_key_label_shift_produces_uppercase() {
        let mut state = XkbState::from_layout_name(Some("English (US)")).unwrap();
        state.update_key(Key::KEY_LEFTSHIFT, true);
        assert_eq!(key_label_from_layout(Key::KEY_A, &state), "A");
        assert_eq!(key_label_from_layout(Key::KEY_2, &state), "@");
    }
}
