use crate::domain::config::{KeystrokeConfig, SuperKeyIconConfig, SuperKeyIconMode};
use crate::input::{is_layout_key, is_modifier, is_super_key, KeyDisplay, XkbState};
use evdev::Key;
use gtk4::gdk::Texture;
use gtk4::gdk_pixbuf::Pixbuf;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Image, Label, Orientation, Widget};
use std::collections::{HashSet, VecDeque};
use std::time::{Duration, Instant};
use tracing::warn;

const LOGO_SVG: &[u8] = include_bytes!("../assets/logo-symbolic.svg");

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

fn is_command_modifier(key: Key) -> bool {
    matches!(
        key,
        Key::KEY_LEFTCTRL
            | Key::KEY_RIGHTCTRL
            | Key::KEY_LEFTALT
            | Key::KEY_RIGHTALT
            | Key::KEY_LEFTMETA
            | Key::KEY_RIGHTMETA
    )
}

fn unified_char(key: Key, xkb_state: &XkbState) -> Option<char> {
    if !is_layout_key(key) {
        return None;
    }

    let utf8 = xkb_state.key_get_utf8(key)?;
    let mut chars = utf8.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) if !c.is_control() => Some(c),
        _ => None,
    }
}

/// Only `backspace` edits the grouped word; `delete` renders as its own key.
fn unify_edits_word(key: Key) -> bool {
    matches!(key, Key::KEY_BACKSPACE)
}

fn is_word_break_key(key: Key) -> bool {
    matches!(
        key,
        Key::KEY_SPACE | Key::KEY_TAB | Key::KEY_ENTER | Key::KEY_KPENTER
    )
}

pub fn hibiki_logo_texture(size: i32) -> Option<Texture> {
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

    Pixbuf::from_stream_at_scale(&stream, size, size, false, gtk4::gio::Cancellable::NONE)
        .ok()
        .map(|pixbuf| Texture::for_pixbuf(&pixbuf))
}

/// Only alpha-capable PNG/WebP is accepted so the bubble shows through.
fn load_super_key_image(config: &SuperKeyIconConfig) -> Option<Texture> {
    if config.mode != SuperKeyIconMode::Image {
        return None;
    }

    let path = config.image_path.as_deref()?;

    if !matches!(
        std::path::Path::new(path)
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("png" | "webp")
    ) {
        warn!("Super key image {:?} is not a PNG or WebP file", path);
        return None;
    }

    match Pixbuf::from_file(path) {
        Ok(pixbuf) => {
            if !pixbuf.has_alpha() {
                warn!("Super key image {:?} has no transparency; ignoring", path);
                return None;
            }
            Some(Texture::for_pixbuf(&pixbuf))
        }
        Err(e) => {
            warn!("Failed to load super key image {:?}: {}", path, e);
            None
        }
    }
}

#[derive(Debug)]
struct DisplayedKey {
    keys: Vec<Key>,

    text: Option<String>,

    count: usize,

    last_active: Instant,

    is_held: bool,

    /// Cleared when a modifier is released so a re-press starts a fresh bubble.
    extendable: bool,

    widget: Widget,
}

struct DisplayedKeyState {
    keys: Vec<Key>,
    text: Option<String>,
    count: usize,
    last_active: Instant,
    is_held: bool,
    extendable: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum MergeAction {
    Extend,
    Increment,
    New,
}

fn decide_merge(
    last_keys: &[Key],
    last_extendable: bool,
    current_combo: &[Key],
    held_modifiers: &HashSet<Key>,
    separate_duplicate_keys: bool,
    combine_modifier_keys: bool,
    max_combo_keys: usize,
) -> MergeAction {
    let is_duplicate = last_keys == current_combo;

    let combined_keys = last_keys.iter().filter(|k| !is_modifier(**k)).count();
    let can_extend = combine_modifier_keys
        && last_extendable
        && combined_keys < max_combo_keys
        && !held_modifiers.is_empty()
        && held_modifiers.iter().all(|m| last_keys.contains(m));

    if can_extend {
        return MergeAction::Extend;
    }

    if is_duplicate && !separate_duplicate_keys {
        return MergeAction::Increment;
    }

    MergeAction::New
}

pub struct KeyDisplayWidget {
    container: GtkBox,

    displayed_keys: VecDeque<DisplayedKey>,

    held_modifiers: HashSet<Key>,

    held_keys: HashSet<Key>,

    max_keys: usize,

    display_duration: Duration,

    separate_duplicate_keys: bool,

    combine_modifier_keys: bool,

    max_combo_keys: usize,

    unify_words: bool,

    max_word_length: usize,

    backspace_edits_word: bool,

    super_key_icon: SuperKeyIconConfig,

    logo_texture: Option<Texture>,

    super_key_image: Option<Texture>,

    xkb_state: XkbState,
}

impl KeyDisplayWidget {
    pub fn new(config: &KeystrokeConfig) -> Self {
        let container = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .halign(gtk4::Align::Start)
            .valign(gtk4::Align::Center)
            .build();

        container.add_css_class("keystroke-container");

        let logo_texture = hibiki_logo_texture(18);
        let super_key_image = load_super_key_image(&config.super_key_icon);

        Self {
            container,
            displayed_keys: VecDeque::new(),
            held_modifiers: HashSet::new(),
            held_keys: HashSet::new(),
            max_keys: config.max_keys,
            display_duration: Duration::from_millis(config.display_timeout_ms),
            separate_duplicate_keys: config.separate_duplicate_keys,
            combine_modifier_keys: config.combine_modifier_keys,
            max_combo_keys: config.max_combo_keys,
            unify_words: config.unify_words,
            max_word_length: config.max_word_length,
            backspace_edits_word: config.backspace_edits_word,
            super_key_icon: config.super_key_icon.clone(),
            logo_texture,
            super_key_image,
            xkb_state: XkbState::default(),
        }
    }

    pub fn widget(&self) -> &GtkBox {
        &self.container
    }

    pub fn set_layout(&mut self, layout_name: &str) {
        self.xkb_state.set_layout(layout_name);
    }

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

        if key.is_repeat {
            if self.unify_words && is_word_break_key(key.key) {
                self.unify_word_break();
            } else {
                self.refresh_repeat(key.key);
            }
            return;
        }

        self.remove_expired();

        if self.unify_words && self.try_unify(key.key) {
            return;
        }

        let current_combo = self.get_current_combo(key.key);

        match self.merge_action(&current_combo) {
            MergeAction::Increment => {
                if let Some(last) = self.displayed_keys.back() {
                    let (keys, count) = (last.keys.clone(), last.count + 1);
                    self.replace_last(keys, count);
                }
                return;
            }
            MergeAction::Extend => {
                if let Some(last) = self.displayed_keys.back() {
                    let mut keys = last.keys.clone();
                    keys.push(key.key);
                    let count = last.count;
                    self.replace_last(keys, count);
                }
                return;
            }
            MergeAction::New => {}
        }

        while self.displayed_keys.len() >= self.max_keys {
            if let Some(old) = self.displayed_keys.pop_front() {
                self.container.remove(&old.widget);
            }
        }

        let widget = self.create_combo_widget(&current_combo, 1);

        self.container.append(&widget);

        let displayed = DisplayedKey {
            keys: current_combo,
            text: None,
            count: 1,
            last_active: Instant::now(),
            is_held: true,
            extendable: true,
            widget,
        };

        self.displayed_keys.push_back(displayed);
    }

    /// Returns `true` when the key was consumed as typed text rather than shown.
    fn try_unify(&mut self, key: Key) -> bool {
        if self.held_modifiers.iter().any(|m| is_command_modifier(*m)) {
            return false;
        }

        if unify_edits_word(key) {
            if self.backspace_edits_word {
                return self.unify_backspace();
            }
            // Backspace is shown as a key, but close the current word so the
            // next character starts a fresh bubble.
            if let Some(last) = self.displayed_keys.back_mut() {
                if last.text.is_some() {
                    last.extendable = false;
                    last.last_active = Instant::now();
                }
            }
            return false;
        }
        if is_word_break_key(key) {
            return self.unify_word_break();
        }

        let Some(ch) = unified_char(key, &self.xkb_state) else {
            return false;
        };

        if ch.is_whitespace() {
            return self.unify_word_break();
        }

        self.unify_append(ch);
        true
    }

    fn unify_append(&mut self, ch: char) {
        let extend = self
            .displayed_keys
            .back()
            .is_some_and(|d| d.text.is_some() && d.extendable);

        if extend {
            let mut text = self
                .displayed_keys
                .back()
                .and_then(|d| d.text.clone())
                .unwrap_or_default();

            if text.chars().count() >= self.max_word_length {
                if let Some(last) = self.displayed_keys.back_mut() {
                    last.extendable = false;
                    last.last_active = Instant::now();
                }
                self.push_new_word(ch.to_string());
                return;
            }

            text.push(ch);
            self.replace_last_word(text);
        } else {
            self.push_new_word(ch.to_string());
        }
    }

    /// Consumed even when repeating, so the separator never renders as a key.
    fn unify_word_break(&mut self) -> bool {
        let has_word = self
            .displayed_keys
            .back()
            .is_some_and(|d| d.text.is_some() && d.extendable);

        if has_word {
            if let Some(last) = self.displayed_keys.back_mut() {
                last.extendable = false;
                last.last_active = Instant::now();
            }
        }

        true
    }

    fn unify_backspace(&mut self) -> bool {
        let is_word = self.displayed_keys.back().is_some_and(|d| d.text.is_some());

        if !is_word {
            return false;
        }

        let mut text = self
            .displayed_keys
            .back()
            .and_then(|d| d.text.clone())
            .unwrap_or_default();
        text.pop();

        if text.is_empty() {
            if let Some(old) = self.displayed_keys.pop_back() {
                self.container.remove(&old.widget);
            }
        } else {
            self.replace_last_word(text);
        }

        true
    }

    /// Keeps a word bubble from expiring while a key is still auto-repeating.
    fn refresh_repeat(&mut self, key: Key) {
        if let Some(displayed) = self.displayed_keys.back_mut() {
            if displayed.text.is_some() {
                displayed.last_active = Instant::now();
                return;
            }

            if displayed.keys.last() == Some(&key) {
                displayed.last_active = Instant::now();
                displayed.is_held = true;
                displayed.widget.remove_css_class("fading");
            }
        }
    }

    fn merge_action(&self, current_combo: &[Key]) -> MergeAction {
        match self.displayed_keys.back() {
            Some(last) if last.text.is_some() => MergeAction::New,
            Some(last) => decide_merge(
                &last.keys,
                last.extendable,
                current_combo,
                &self.held_modifiers,
                self.separate_duplicate_keys,
                self.combine_modifier_keys,
                self.max_combo_keys,
            ),
            None => MergeAction::New,
        }
    }

    fn replace_last(&mut self, keys: Vec<Key>, count: usize) {
        let Some(old) = self.displayed_keys.pop_back() else {
            return;
        };

        self.container.remove(&old.widget);

        let widget = self.create_combo_widget(&keys, count);
        self.container.append(&widget);

        self.displayed_keys.push_back(DisplayedKey {
            keys,
            text: None,
            count,
            last_active: Instant::now(),
            is_held: true,
            extendable: old.extendable,
            widget,
        });
    }

    fn replace_last_word(&mut self, text: String) {
        let Some(old) = self.displayed_keys.pop_back() else {
            return;
        };

        self.container.remove(&old.widget);

        let widget = self.create_word_widget(&text);
        self.container.append(&widget);

        self.displayed_keys.push_back(DisplayedKey {
            keys: Vec::new(),
            text: Some(text),
            count: 1,
            last_active: Instant::now(),
            is_held: false,
            extendable: old.extendable,
            widget,
        });
    }

    fn push_new_word(&mut self, text: String) {
        while self.displayed_keys.len() >= self.max_keys {
            if let Some(old) = self.displayed_keys.pop_front() {
                self.container.remove(&old.widget);
            }
        }

        let widget = self.create_word_widget(&text);

        self.container.append(&widget);

        self.displayed_keys.push_back(DisplayedKey {
            keys: Vec::new(),
            text: Some(text),
            count: 1,
            last_active: Instant::now(),
            is_held: false,
            extendable: true,
            widget,
        });
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

    fn create_combo_widget(&self, keys: &[Key], count: usize) -> Widget {
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
                let image = self.create_super_key_image();
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

        if count > 1 {
            let count_label = Label::builder()
                .label(format!("×{count}"))
                .halign(gtk4::Align::Center)
                .valign(gtk4::Align::Start)
                .css_classes(["combo-count"])
                .build();
            key_box.append(&count_label);
        }

        key_box.upcast()
    }

    fn create_super_key_image(&self) -> Image {
        match self.super_key_icon.mode {
            SuperKeyIconMode::Hibiki => self.logo_image(),
            SuperKeyIconMode::Symbolic | SuperKeyIconMode::Icon => self
                .super_key_icon
                .icon_name
                .as_deref()
                .filter(|name| !name.is_empty())
                .map_or_else(
                    || self.logo_image(),
                    |name| {
                        let image = Image::from_icon_name(name);
                        image.set_pixel_size(18);
                        image
                    },
                ),
            SuperKeyIconMode::Image => self.super_key_image.as_ref().map_or_else(
                || self.logo_image(),
                |texture| {
                    let image = Image::from_paintable(Some(texture));
                    image.set_pixel_size(18);
                    image
                },
            ),
        }
    }

    fn logo_image(&self) -> Image {
        if let Some(texture) = &self.logo_texture {
            let image = Image::from_paintable(Some(texture));
            image.set_pixel_size(18);
            image
        } else {
            Image::new()
        }
    }

    fn refresh_widgets(&mut self) {
        let entries: Vec<DisplayedKeyState> = self
            .displayed_keys
            .iter()
            .map(|d| DisplayedKeyState {
                keys: d.keys.clone(),
                text: d.text.clone(),
                count: d.count,
                last_active: d.last_active,
                is_held: d.is_held,
                extendable: d.extendable,
            })
            .collect();

        while let Some(child) = self.container.first_child() {
            self.container.remove(&child);
        }
        self.displayed_keys.clear();

        for state in entries {
            let widget = match &state.text {
                Some(text) => self.create_word_widget(text),
                None => self.create_combo_widget(&state.keys, state.count),
            };
            self.container.append(&widget);

            self.displayed_keys.push_back(DisplayedKey {
                keys: state.keys,
                text: state.text,
                count: state.count,
                last_active: state.last_active,
                is_held: state.is_held,
                extendable: state.extendable,
                widget,
            });
        }
    }

    fn create_word_widget(&self, text: &str) -> Widget {
        let key_box = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(6)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .hexpand(false)
            .build();
        key_box.add_css_class("keystroke-key");
        key_box.add_css_class("word");

        let label = Label::builder()
            .label(text)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .hexpand(true)
            .build();
        key_box.append(&label);

        key_box.upcast()
    }

    pub fn remove_key(&mut self, key: &KeyDisplay) {
        self.held_keys.remove(&key.key);

        if is_modifier(key.key) {
            self.held_modifiers.remove(&key.key);
        }

        self.xkb_state.update_key(key.key, false);

        let released_modifier = is_modifier(key.key);
        for displayed in self.displayed_keys.iter_mut() {
            if displayed.keys.contains(&key.key) {
                displayed.is_held = false;
                displayed.last_active = Instant::now();
                displayed.widget.add_css_class("fading");
                if released_modifier {
                    displayed.extendable = false;
                }
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
    pub fn set_separate_duplicate_keys(&mut self, separate: bool) {
        self.separate_duplicate_keys = separate;
    }

    #[allow(dead_code)]
    pub fn set_combine_modifier_keys(&mut self, combine: bool) {
        self.combine_modifier_keys = combine;
    }

    #[allow(dead_code)]
    pub fn set_max_combo_keys(&mut self, max_combo_keys: usize) {
        self.max_combo_keys = max_combo_keys;
    }

    #[allow(dead_code)]
    pub fn set_unify_words(&mut self, unify: bool) {
        self.unify_words = unify;
    }

    #[allow(dead_code)]
    pub fn set_backspace_edits_word(&mut self, edits: bool) {
        self.backspace_edits_word = edits;
    }

    #[allow(dead_code)]
    pub fn set_max_word_length(&mut self, max_word_length: usize) {
        self.max_word_length = max_word_length.max(1);
    }

    pub fn set_super_key_icon(&mut self, config: &SuperKeyIconConfig) {
        if self.super_key_icon == *config {
            return;
        }

        self.super_key_icon = config.clone();
        self.super_key_image = load_super_key_image(config);
        self.refresh_widgets();
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

    fn modifiers(keys: &[Key]) -> HashSet<Key> {
        keys.iter().copied().collect()
    }

    const MAX: usize = 4;

    #[test]
    fn duplicate_keys_are_separate_by_default() {
        let last = [Key::KEY_F];
        let action = decide_merge(&last, true, &last, &HashSet::new(), true, false, MAX);
        assert_eq!(action, MergeAction::New);
    }

    #[test]
    fn duplicate_keys_are_grouped_when_disabled() {
        let last = [Key::KEY_F];
        let action = decide_merge(&last, true, &last, &HashSet::new(), false, false, MAX);
        assert_eq!(action, MergeAction::Increment);
    }

    #[test]
    fn held_modifier_extends_trailing_bubble() {
        let last = [Key::KEY_LEFTCTRL, Key::KEY_A];
        let combo = [Key::KEY_LEFTCTRL, Key::KEY_B];
        let action = decide_merge(
            &last,
            true,
            &combo,
            &modifiers(&[Key::KEY_LEFTCTRL]),
            true,
            true,
            MAX,
        );
        assert_eq!(action, MergeAction::Extend);
    }

    #[test]
    fn held_modifier_combines_repeated_keys() {
        let last = [Key::KEY_LEFTCTRL, Key::KEY_C];
        let combo = [Key::KEY_LEFTCTRL, Key::KEY_C];
        let action = decide_merge(
            &last,
            true,
            &combo,
            &modifiers(&[Key::KEY_LEFTCTRL]),
            true,
            true,
            MAX,
        );
        assert_eq!(action, MergeAction::Extend);
    }

    #[test]
    fn repeated_keys_without_modifier_stay_separate() {
        let last = [Key::KEY_F];
        let combo = [Key::KEY_F];
        let action = decide_merge(&last, true, &combo, &HashSet::new(), true, true, MAX);
        assert_eq!(action, MergeAction::New);
    }

    #[test]
    fn held_modifier_stops_extending_at_the_cap() {
        let last = [
            Key::KEY_LEFTCTRL,
            Key::KEY_A,
            Key::KEY_B,
            Key::KEY_C,
            Key::KEY_D,
        ];
        let combo = [Key::KEY_LEFTCTRL, Key::KEY_E];
        let action = decide_merge(
            &last,
            true,
            &combo,
            &modifiers(&[Key::KEY_LEFTCTRL]),
            true,
            true,
            MAX,
        );
        assert_eq!(action, MergeAction::New);
    }

    #[test]
    fn released_modifier_starts_a_new_bubble() {
        let last = [Key::KEY_LEFTCTRL, Key::KEY_A];
        let combo = [Key::KEY_B];
        let action = decide_merge(&last, true, &combo, &HashSet::new(), true, true, MAX);
        assert_eq!(action, MergeAction::New);

        let combo = [Key::KEY_LEFTCTRL, Key::KEY_B];
        let action = decide_merge(
            &last,
            false,
            &combo,
            &modifiers(&[Key::KEY_LEFTCTRL]),
            true,
            true,
            MAX,
        );
        assert_eq!(action, MergeAction::New);
    }

    #[test]
    fn modifier_combine_is_off_by_default() {
        let last = [Key::KEY_LEFTCTRL, Key::KEY_A];
        let combo = [Key::KEY_LEFTCTRL, Key::KEY_B];
        let action = decide_merge(
            &last,
            true,
            &combo,
            &modifiers(&[Key::KEY_LEFTCTRL]),
            true,
            false,
            MAX,
        );
        assert_eq!(action, MergeAction::New);
    }

    #[test]
    fn adding_a_modifier_starts_a_new_bubble() {
        let last = [Key::KEY_LEFTCTRL, Key::KEY_A];
        let combo = [Key::KEY_LEFTCTRL, Key::KEY_LEFTSHIFT, Key::KEY_B];
        let action = decide_merge(
            &last,
            true,
            &combo,
            &modifiers(&[Key::KEY_LEFTCTRL, Key::KEY_LEFTSHIFT]),
            true,
            true,
            MAX,
        );
        assert_eq!(action, MergeAction::New);
    }

    #[test]
    fn command_modifiers_are_detected() {
        assert!(is_command_modifier(Key::KEY_LEFTCTRL));
        assert!(is_command_modifier(Key::KEY_RIGHTCTRL));
        assert!(is_command_modifier(Key::KEY_LEFTALT));
        assert!(is_command_modifier(Key::KEY_RIGHTALT));
        assert!(is_command_modifier(Key::KEY_LEFTMETA));
        assert!(is_command_modifier(Key::KEY_RIGHTMETA));

        assert!(!is_command_modifier(Key::KEY_LEFTSHIFT));
        assert!(!is_command_modifier(Key::KEY_RIGHTSHIFT));
        assert!(!is_command_modifier(Key::KEY_A));
        assert!(!is_command_modifier(Key::KEY_SPACE));
    }

    #[test]
    fn word_break_keys_are_detected() {
        assert!(is_word_break_key(Key::KEY_SPACE));
        assert!(is_word_break_key(Key::KEY_TAB));
        assert!(is_word_break_key(Key::KEY_ENTER));
        assert!(is_word_break_key(Key::KEY_KPENTER));

        assert!(!is_word_break_key(Key::KEY_A));
        assert!(!is_word_break_key(Key::KEY_BACKSPACE));
        assert!(!is_word_break_key(Key::KEY_LEFTCTRL));
    }

    #[test]
    fn only_backspace_edits_a_unified_word() {
        assert!(unify_edits_word(Key::KEY_BACKSPACE));
        assert!(!unify_edits_word(Key::KEY_DELETE));
        assert!(!unify_edits_word(Key::KEY_A));
        // Delete must be shown as its own key, not erase the grouped word.
        assert!(!is_word_break_key(Key::KEY_DELETE));
    }

    #[test]
    fn unified_char_uses_layout() {
        let state = XkbState::from_layout_name(Some("English (US)")).unwrap();
        assert_eq!(unified_char(Key::KEY_A, &state), Some('a'));
        assert_eq!(unified_char(Key::KEY_1, &state), Some('1'));
        assert_eq!(unified_char(Key::KEY_SEMICOLON, &state), Some(';'));

        assert_eq!(unified_char(Key::KEY_F1, &state), None);
        assert_eq!(unified_char(Key::KEY_LEFTCTRL, &state), None);
        assert_eq!(unified_char(Key::KEY_SPACE, &state), None);
    }

    #[test]
    fn unified_char_respects_shift() {
        let mut state = XkbState::from_layout_name(Some("English (US)")).unwrap();
        state.update_key(Key::KEY_LEFTSHIFT, true);
        assert_eq!(unified_char(Key::KEY_A, &state), Some('A'));
        assert_eq!(unified_char(Key::KEY_1, &state), Some('!'));
    }

    #[test]
    fn unified_char_respects_layout_remap() {
        let state = XkbState::from_layout_name(Some("German")).unwrap();
        assert_eq!(unified_char(Key::KEY_Y, &state), Some('z'));
        assert_eq!(unified_char(Key::KEY_Z, &state), Some('y'));
    }
}
