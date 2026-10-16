use crate::domain::Position;
use crate::input::{KeyDisplay, XkbState};
use evdev::Key;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Label, Orientation};
use std::collections::{HashMap, VecDeque};
use std::time::{Duration, Instant};

const MAX_BUBBLES: usize = 10;
const MAX_CHARS_PER_LINE: usize = 45;
const NEW_BUBBLE_TIMEOUT_MS: u64 = 3000;

#[must_use]
pub fn alignment_for(position: Position) -> gtk4::Align {
    match position {
        Position::TopLeft | Position::MiddleLeft | Position::BottomLeft => gtk4::Align::Start,
        Position::TopRight | Position::MiddleRight | Position::BottomRight => gtk4::Align::End,
        Position::TopCenter | Position::Center | Position::BottomCenter => gtk4::Align::Center,
    }
}

#[derive(Default)]
struct ModifierState {
    ctrl: bool,
    alt: bool,
    super_key: bool,
}

impl ModifierState {
    fn update(&mut self, key: Key, pressed: bool) {
        match key {
            Key::KEY_LEFTCTRL | Key::KEY_RIGHTCTRL => self.ctrl = pressed,
            Key::KEY_LEFTALT | Key::KEY_RIGHTALT => self.alt = pressed,
            Key::KEY_LEFTMETA | Key::KEY_RIGHTMETA => self.super_key = pressed,
            _ => {}
        }
    }

    fn has_command_modifier(&self) -> bool {
        self.ctrl || self.alt || self.super_key
    }
}

struct ChatBubble {
    text: String,
    last_modified: Instant,
    label: Label,
}

impl ChatBubble {
    fn new(align: gtk4::Align) -> Self {
        let label = Label::builder()
            .wrap(true)
            .wrap_mode(gtk4::pango::WrapMode::WordChar)
            .max_width_chars(MAX_CHARS_PER_LINE as i32)
            .xalign(0.0)
            .halign(align)
            .build();
        label.add_css_class("bubble");

        Self {
            text: String::with_capacity(64),
            last_modified: Instant::now(),
            label,
        }
    }

    fn set_align(&self, align: gtk4::Align) {
        self.label.set_halign(align);
    }

    /// Keeps the bubble at the widest it has ever been. A wrapping label's
    /// natural width is its longest wrapped line, so without this the bubble
    /// shrinks (and jumps) every time a word reflows onto a new line.
    fn pin_width(&self) {
        let (_, natural, _, _) = self.label.measure(gtk4::Orientation::Horizontal, -1);
        self.label.set_width_request(natural);
    }

    fn append_char(&mut self, c: char) {
        self.text.push(c);
        self.label.set_text(&self.text);
        self.pin_width();
        self.last_modified = Instant::now();
    }

    fn backspace(&mut self) {
        self.text.pop();
        self.label.set_text(&self.text);
        self.pin_width();
        self.last_modified = Instant::now();
    }

    fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    fn widget(&self) -> &Label {
        &self.label
    }

    fn is_stale(&self, timeout: Duration) -> bool {
        self.last_modified.elapsed() > timeout
    }
}

fn is_modifier_key(key: Key) -> bool {
    matches!(
        key,
        Key::KEY_LEFTCTRL
            | Key::KEY_RIGHTCTRL
            | Key::KEY_LEFTALT
            | Key::KEY_RIGHTALT
            | Key::KEY_LEFTMETA
            | Key::KEY_RIGHTMETA
            | Key::KEY_LEFTSHIFT
            | Key::KEY_RIGHTSHIFT
    )
}

fn is_ignored_key(key: Key) -> bool {
    matches!(
        key,
        Key::KEY_LEFTCTRL
            | Key::KEY_RIGHTCTRL
            | Key::KEY_LEFTALT
            | Key::KEY_RIGHTALT
            | Key::KEY_LEFTMETA
            | Key::KEY_RIGHTMETA
            | Key::KEY_LEFTSHIFT
            | Key::KEY_RIGHTSHIFT
            | Key::KEY_CAPSLOCK
            | Key::KEY_NUMLOCK
            | Key::KEY_SCROLLLOCK
            | Key::KEY_FN
            | Key::KEY_ESC
            | Key::KEY_INSERT
            | Key::KEY_HOME
            | Key::KEY_END
            | Key::KEY_PAGEUP
            | Key::KEY_PAGEDOWN
            | Key::KEY_UP
            | Key::KEY_DOWN
            | Key::KEY_LEFT
            | Key::KEY_RIGHT
            | Key::KEY_PRINT
            | Key::KEY_PAUSE
            | Key::KEY_F1
            | Key::KEY_F2
            | Key::KEY_F3
            | Key::KEY_F4
            | Key::KEY_F5
            | Key::KEY_F6
            | Key::KEY_F7
            | Key::KEY_F8
            | Key::KEY_F9
            | Key::KEY_F10
            | Key::KEY_F11
            | Key::KEY_F12
    )
}

fn key_to_char(key: Key, xkb_state: &XkbState) -> Option<BubbleInput> {
    match key {
        Key::KEY_ENTER | Key::KEY_KPENTER => return Some(BubbleInput::NewLine),
        Key::KEY_BACKSPACE => return Some(BubbleInput::Backspace),
        Key::KEY_DELETE => return Some(BubbleInput::Backspace),
        _ => {}
    }

    if is_ignored_key(key) {
        return None;
    }

    if let Some(utf8) = xkb_state.key_get_utf8(key) {
        let c = utf8.chars().next()?;
        if c.is_control() && c != ' ' && c != '\t' {
            return None;
        }

        if c == '\t' {
            return Some(BubbleInput::Char(' '));
        }
        return Some(BubbleInput::Char(c));
    }

    None
}

#[derive(Debug)]
enum BubbleInput {
    Char(char),
    Backspace,
    NewLine,
}

pub struct BubbleDisplayWidget {
    container: GtkBox,
    bubbles: VecDeque<ChatBubble>,
    display_duration: Duration,
    new_bubble_timeout: Duration,
    modifiers: ModifierState,
    want_new_bubble: bool,
    xkb_state: XkbState,
    pressed_keys: HashMap<Key, u32>,
    align: gtk4::Align,
}

impl BubbleDisplayWidget {
    pub fn new(display_timeout_ms: u64) -> Self {
        let container = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .spacing(6)
            .halign(gtk4::Align::Start)
            .valign(gtk4::Align::End)
            .build();

        container.add_css_class("bubble-container");

        Self {
            container,
            bubbles: VecDeque::with_capacity(MAX_BUBBLES),
            display_duration: Duration::from_millis(display_timeout_ms),
            new_bubble_timeout: Duration::from_millis(NEW_BUBBLE_TIMEOUT_MS),
            modifiers: ModifierState::default(),
            want_new_bubble: false,
            xkb_state: XkbState::default(),
            pressed_keys: HashMap::new(),
            align: gtk4::Align::Start,
        }
    }

    pub fn set_alignment(&mut self, align: gtk4::Align) {
        if self.align == align {
            return;
        }

        self.align = align;
        self.container.set_halign(align);
        for bubble in &self.bubbles {
            bubble.set_align(align);
        }
    }

    #[allow(dead_code)]
    pub fn with_layout(display_timeout_ms: u64, layout_name: &str) -> Self {
        let mut widget = Self::new(display_timeout_ms);
        widget.set_layout(layout_name);
        widget
    }

    pub fn set_display_duration(&mut self, timeout_ms: u64) {
        self.display_duration = Duration::from_millis(timeout_ms);
    }

    pub fn set_layout(&mut self, layout_name: &str) {
        self.xkb_state.set_layout(layout_name);
    }

    #[allow(dead_code)]
    pub fn layout_name(&self) -> &str {
        self.xkb_state.layout_name()
    }

    pub fn widget(&self) -> &GtkBox {
        &self.container
    }

    pub fn process_key(&mut self, key: KeyDisplay) {
        if is_modifier_key(key.key) {
            if !key.is_repeat {
                self.xkb_state.update_key(key.key, true);
                self.modifiers.update(key.key, true);
            }
            return;
        }

        if !key.is_repeat {
            self.xkb_state.update_key(key.key, true);
            let count = self.pressed_keys.entry(key.key).or_insert(0);
            *count += 1;
        }

        self.modifiers.update(key.key, true);

        if self.modifiers.has_command_modifier() {
            return;
        }

        let input = match key_to_char(key.key, &self.xkb_state) {
            Some(input) => input,
            None => return,
        };

        match input {
            BubbleInput::Char(c) => {
                if self.want_new_bubble {
                    self.want_new_bubble = false;
                    if self.bubbles.back().is_some_and(|b| !b.is_empty()) {
                        self.create_new_bubble();
                    }
                }
                self.ensure_active_bubble();
                if let Some(bubble) = self.bubbles.back_mut() {
                    bubble.append_char(c);
                }
            }
            BubbleInput::Backspace => {
                if let Some(bubble) = self.bubbles.back_mut() {
                    if bubble.is_empty() {
                        if let Some(removed) = self.bubbles.pop_back() {
                            self.container.remove(removed.widget());
                        }

                        if let Some(prev_bubble) = self.bubbles.back_mut() {
                            prev_bubble.backspace();
                        }
                    } else {
                        bubble.backspace();
                    }
                }
            }
            BubbleInput::NewLine => {
                if self.xkb_state.is_shift_active() {
                    self.ensure_active_bubble();
                    if let Some(bubble) = self.bubbles.back_mut() {
                        bubble.append_char('\n');
                    }
                } else if self.bubbles.back().is_some_and(|b| !b.is_empty()) {
                    self.want_new_bubble = true;
                }
            }
        }
    }

    pub fn process_key_release(&mut self, key: KeyDisplay) {
        if is_modifier_key(key.key) {
            self.xkb_state.update_key(key.key, false);
            self.modifiers.update(key.key, false);
            return;
        }

        if let Some(count) = self.pressed_keys.get_mut(&key.key) {
            if *count > 0 {
                *count -= 1;
            }

            if *count == 0 {
                self.pressed_keys.remove(&key.key);
                self.xkb_state.update_key(key.key, false);
            }
        } else {
            self.xkb_state.update_key(key.key, false);
        }
    }

    fn ensure_active_bubble(&mut self) {
        let need_new_bubble = if let Some(bubble) = self.bubbles.back() {
            bubble.is_stale(self.new_bubble_timeout) && !bubble.is_empty()
        } else {
            true
        };

        if need_new_bubble {
            self.create_new_bubble();
        }
    }

    fn create_new_bubble(&mut self) {
        while self.bubbles.len() >= MAX_BUBBLES {
            if let Some(removed) = self.bubbles.pop_front() {
                self.container.remove(removed.widget());
            }
        }

        let bubble = ChatBubble::new(self.align);
        self.container.append(bubble.widget());
        self.bubbles.push_back(bubble);
    }

    pub fn remove_expired(&mut self) {
        let duration = self.display_duration;

        while self.bubbles.len() > 1 {
            if let Some(bubble) = self.bubbles.front() {
                if bubble.last_modified.elapsed() > duration {
                    if let Some(removed) = self.bubbles.pop_front() {
                        self.container.remove(removed.widget());
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        while let Some(bubble) = self.bubbles.pop_front() {
            self.container.remove(bubble.widget());
        }
    }

    #[allow(dead_code)]
    pub fn has_content(&self) -> bool {
        self.bubbles.iter().any(|b| !b.is_empty())
    }

    pub fn should_show(&self) -> bool {
        if self.bubbles.is_empty() {
            return false;
        }

        self.bubbles
            .iter()
            .any(|b| !b.is_empty() && !b.is_stale(self.display_duration))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alignment_for_position() {
        assert_eq!(alignment_for(Position::BottomLeft), gtk4::Align::Start);
        assert_eq!(alignment_for(Position::MiddleRight), gtk4::Align::End);
        assert_eq!(alignment_for(Position::Center), gtk4::Align::Center);
    }

    #[test]
    fn test_is_ignored_key() {
        assert!(is_ignored_key(Key::KEY_LEFTCTRL));
        assert!(is_ignored_key(Key::KEY_RIGHTALT));
        assert!(is_ignored_key(Key::KEY_LEFTMETA));
        assert!(is_ignored_key(Key::KEY_F1));
        assert!(!is_ignored_key(Key::KEY_A));
        assert!(!is_ignored_key(Key::KEY_SPACE));
    }

    #[test]
    fn test_key_to_char_regular_keys() {
        let xkb_state = XkbState::from_layout_name(Some("English (US)")).unwrap();
        let result = key_to_char(Key::KEY_A, &xkb_state);
        assert!(matches!(result, Some(BubbleInput::Char('a'))));
        let result = key_to_char(Key::KEY_1, &xkb_state);
        assert!(matches!(result, Some(BubbleInput::Char('1'))));
    }

    #[test]
    fn test_key_to_char_with_shift() {
        let mut xkb_state = XkbState::from_layout_name(Some("English (US)")).unwrap();
        xkb_state.update_key(Key::KEY_LEFTSHIFT, true);
        let result = key_to_char(Key::KEY_A, &xkb_state);
        assert!(matches!(result, Some(BubbleInput::Char('A'))));
        let result = key_to_char(Key::KEY_1, &xkb_state);
        assert!(matches!(result, Some(BubbleInput::Char('!'))));
    }

    #[test]
    fn test_shift_persists_after_character_release() {
        let mut xkb_state = XkbState::from_layout_name(Some("English (US)")).unwrap();
        xkb_state.update_key(Key::KEY_LEFTSHIFT, true);
        assert!(xkb_state.is_shift_active());
        xkb_state.update_key(Key::KEY_A, true);
        let result = key_to_char(Key::KEY_A, &xkb_state);
        assert!(matches!(result, Some(BubbleInput::Char('A'))));
        xkb_state.update_key(Key::KEY_A, false);
        assert!(xkb_state.is_shift_active());
        xkb_state.update_key(Key::KEY_B, true);
        let result = key_to_char(Key::KEY_B, &xkb_state);
        assert!(matches!(result, Some(BubbleInput::Char('B'))));
    }

    #[test]
    fn test_shift_persists_through_multiple_characters() {
        let mut xkb_state = XkbState::from_layout_name(Some("English (US)")).unwrap();
        xkb_state.update_key(Key::KEY_LEFTSHIFT, true);
        let keys = [Key::KEY_H, Key::KEY_E, Key::KEY_L, Key::KEY_L, Key::KEY_O];
        let expected = ['H', 'E', 'L', 'L', 'O'];
        for (key, expected_char) in keys.iter().zip(expected.iter()) {
            xkb_state.update_key(*key, true);
            let result = key_to_char(*key, &xkb_state);
            if let Some(BubbleInput::Char(c)) = result {
                assert_eq!(c, *expected_char);
            } else {
                panic!("Expected Char but got {:?}", result);
            }
            xkb_state.update_key(*key, false);
            assert!(xkb_state.is_shift_active());
        }
    }

    #[test]
    fn test_right_shift_works_same_as_left() {
        let mut xkb_state = XkbState::from_layout_name(Some("English (US)")).unwrap();
        xkb_state.update_key(Key::KEY_RIGHTSHIFT, true);
        assert!(xkb_state.is_shift_active());
        xkb_state.update_key(Key::KEY_A, true);
        let result = key_to_char(Key::KEY_A, &xkb_state);
        assert!(matches!(result, Some(BubbleInput::Char('A'))));
        xkb_state.update_key(Key::KEY_A, false);
        assert!(xkb_state.is_shift_active());
    }

    #[test]
    fn test_reset_modifiers_clears_shift() {
        let mut xkb_state = XkbState::from_layout_name(Some("English (US)")).unwrap();
        xkb_state.update_key(Key::KEY_LEFTSHIFT, true);
        assert!(xkb_state.is_shift_active());
        xkb_state.reset_modifiers();
        assert!(!xkb_state.is_shift_active());
        let result = key_to_char(Key::KEY_A, &xkb_state);
        assert!(matches!(result, Some(BubbleInput::Char('a'))));
    }
}
