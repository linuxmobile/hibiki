use crate::domain::Command;
use evdev::Key;
use std::collections::HashSet;
use tracing::{info, warn};

pub struct GlobalHotKeyManager {
    pressed_keys: HashSet<Key>,
    capture_hotkey: HotkeyCombo,
    focus_hotkey: HotkeyCombo,
    pause_hotkey: HotkeyCombo,
    toggle_focus_hotkey: HotkeyCombo,
    toggle_mode_hotkey: HotkeyCombo,
}

#[derive(Debug, Clone, Default)]
struct HotkeyCombo {
    modifiers: HashSet<Key>,
    key: Option<Key>,
}

impl Default for GlobalHotKeyManager {
    fn default() -> Self {
        Self::new()
    }
}

impl GlobalHotKeyManager {
    pub fn new() -> Self {
        Self {
            pressed_keys: HashSet::new(),
            capture_hotkey: HotkeyCombo::default(),
            focus_hotkey: HotkeyCombo::default(),
            pause_hotkey: HotkeyCombo::default(),
            toggle_focus_hotkey: HotkeyCombo::default(),
            toggle_mode_hotkey: HotkeyCombo::default(),
        }
    }

    pub fn set_capture_hotkey(&mut self, accelerator: &str) {
        info!("Setting capture hotkey to: {}", accelerator);
        self.capture_hotkey = parse_accelerator(accelerator);
    }

    pub fn set_focus_hotkey(&mut self, accelerator: &str) {
        info!("Setting focus hotkey to: {}", accelerator);
        self.focus_hotkey = parse_accelerator(accelerator);
    }

    pub fn set_pause_hotkey(&mut self, accelerator: &str) {
        info!("Setting pause hotkey to: {}", accelerator);
        self.pause_hotkey = parse_accelerator(accelerator);
    }

    pub fn set_toggle_focus_hotkey(&mut self, accelerator: &str) {
        info!("Setting toggle focus hotkey to: {}", accelerator);
        self.toggle_focus_hotkey = parse_accelerator(accelerator);
    }

    pub fn set_toggle_mode_hotkey(&mut self, accelerator: &str) {
        info!("Setting toggle mode hotkey to: {}", accelerator);
        self.toggle_mode_hotkey = parse_accelerator(accelerator);
    }

    pub fn process(&mut self, key: Key, pressed: bool) -> Option<Command> {
        if pressed {
            self.pressed_keys.insert(key);

            if self.is_combo_matched(&self.capture_hotkey, key) {
                return Some(Command::ActivateKeystrokeMode);
            }

            if self.is_combo_matched(&self.focus_hotkey, key) {
                return Some(Command::ActivateBubbleMode);
            }

            if self.is_combo_matched(&self.toggle_mode_hotkey, key) {
                return Some(Command::ToggleMode);
            }

            if self.is_combo_matched(&self.pause_hotkey, key) {
                return Some(Command::TogglePause);
            }

            if self.is_combo_matched(&self.toggle_focus_hotkey, key) {
                return Some(Command::ToggleFocus);
            }
        } else {
            self.pressed_keys.remove(&key);
        }

        None
    }

    pub fn reset(&mut self) {
        self.pressed_keys.clear();
        info!("GlobalHotKeyManager state reset");
    }

    fn is_combo_matched(&self, combo: &HotkeyCombo, trigger_key: Key) -> bool {
        if let Some(target_key) = combo.key {
            if target_key != trigger_key {
                return false;
            }

            for mod_key in &combo.modifiers {
                let is_pressed = match *mod_key {
                    Key::KEY_LEFTCTRL => {
                        self.pressed_keys.contains(&Key::KEY_LEFTCTRL)
                            || self.pressed_keys.contains(&Key::KEY_RIGHTCTRL)
                    }
                    Key::KEY_LEFTALT => {
                        self.pressed_keys.contains(&Key::KEY_LEFTALT)
                            || self.pressed_keys.contains(&Key::KEY_RIGHTALT)
                    }
                    Key::KEY_LEFTSHIFT => {
                        self.pressed_keys.contains(&Key::KEY_LEFTSHIFT)
                            || self.pressed_keys.contains(&Key::KEY_RIGHTSHIFT)
                    }
                    Key::KEY_LEFTMETA => {
                        self.pressed_keys.contains(&Key::KEY_LEFTMETA)
                            || self.pressed_keys.contains(&Key::KEY_RIGHTMETA)
                    }
                    _ => self.pressed_keys.contains(mod_key),
                };

                if !is_pressed {
                    return false;
                }
            }

            return true;
        }
        false
    }
}

fn parse_accelerator(accel: &str) -> HotkeyCombo {
    let mut combo = HotkeyCombo::default();

    let parts: Vec<&str> = accel.split('>').collect();

    for part in parts {
        if part.is_empty() {
            continue;
        }

        let clean_part = if let Some(stripped) = part.strip_prefix('<') {
            stripped
        } else {
            part
        };

        match clean_part {
            "Control" | "Ctrl" => {
                combo.modifiers.insert(Key::KEY_LEFTCTRL);
            }
            "Alt" => {
                combo.modifiers.insert(Key::KEY_LEFTALT);
            }
            "Shift" => {
                combo.modifiers.insert(Key::KEY_LEFTSHIFT);
            }
            "Super" | "Meta" => {
                combo.modifiers.insert(Key::KEY_LEFTMETA);
            }
            key_name => {
                if let Some(key) = map_key_name(key_name) {
                    combo.key = Some(key);
                } else {
                    warn!("Unknown key in accelerator: {}", key_name);
                }
            }
        }
    }

    combo
}

fn map_key_name(name: &str) -> Option<Key> {
    match name.to_lowercase().as_str() {
        "a" => Some(Key::KEY_A),
        "b" => Some(Key::KEY_B),
        "c" => Some(Key::KEY_C),
        "d" => Some(Key::KEY_D),
        "e" => Some(Key::KEY_E),
        "f" => Some(Key::KEY_F),
        "g" => Some(Key::KEY_G),
        "h" => Some(Key::KEY_H),
        "i" => Some(Key::KEY_I),
        "j" => Some(Key::KEY_J),
        "k" => Some(Key::KEY_K),
        "l" => Some(Key::KEY_L),
        "m" => Some(Key::KEY_M),
        "n" => Some(Key::KEY_N),
        "o" => Some(Key::KEY_O),
        "p" => Some(Key::KEY_P),
        "q" => Some(Key::KEY_Q),
        "r" => Some(Key::KEY_R),
        "s" => Some(Key::KEY_S),
        "t" => Some(Key::KEY_T),
        "u" => Some(Key::KEY_U),
        "v" => Some(Key::KEY_V),
        "w" => Some(Key::KEY_W),
        "x" => Some(Key::KEY_X),
        "y" => Some(Key::KEY_Y),
        "z" => Some(Key::KEY_Z),
        "0" => Some(Key::KEY_0),
        "1" => Some(Key::KEY_1),
        "2" => Some(Key::KEY_2),
        "3" => Some(Key::KEY_3),
        "4" => Some(Key::KEY_4),
        "5" => Some(Key::KEY_5),
        "6" => Some(Key::KEY_6),
        "7" => Some(Key::KEY_7),
        "8" => Some(Key::KEY_8),
        "9" => Some(Key::KEY_9),
        "space" => Some(Key::KEY_SPACE),
        "return" | "enter" => Some(Key::KEY_ENTER),
        "escape" | "esc" => Some(Key::KEY_ESC),
        "backspace" => Some(Key::KEY_BACKSPACE),
        "tab" => Some(Key::KEY_TAB),
        "minus" => Some(Key::KEY_MINUS),
        "equal" => Some(Key::KEY_EQUAL),
        "grave" => Some(Key::KEY_GRAVE),
        "slash" => Some(Key::KEY_SLASH),
        "backslash" => Some(Key::KEY_BACKSLASH),
        "semicolon" => Some(Key::KEY_SEMICOLON),
        "apostrophe" => Some(Key::KEY_APOSTROPHE),
        "comma" => Some(Key::KEY_COMMA),
        "period" | "dot" => Some(Key::KEY_DOT),
        "left" => Some(Key::KEY_LEFT),
        "right" => Some(Key::KEY_RIGHT),
        "up" => Some(Key::KEY_UP),
        "down" => Some(Key::KEY_DOWN),
        "home" => Some(Key::KEY_HOME),
        "end" => Some(Key::KEY_END),
        "page_up" | "pageup" => Some(Key::KEY_PAGEUP),
        "page_down" | "pagedown" => Some(Key::KEY_PAGEDOWN),
        "insert" => Some(Key::KEY_INSERT),
        "delete" => Some(Key::KEY_DELETE),
        "caps_lock" | "capslock" => Some(Key::KEY_CAPSLOCK),
        "print" => Some(Key::KEY_PRINT),
        "pause" => Some(Key::KEY_PAUSE),
        "bracketleft" | "leftbrace" | "[" => Some(Key::KEY_LEFTBRACE),
        "bracketright" | "rightbrace" | "]" => Some(Key::KEY_RIGHTBRACE),
        "greater" => Some(Key::KEY_DOT),
        "less" => Some(Key::KEY_COMMA),
        "f1" => Some(Key::KEY_F1),
        "f2" => Some(Key::KEY_F2),
        "f3" => Some(Key::KEY_F3),
        "f4" => Some(Key::KEY_F4),
        "f5" => Some(Key::KEY_F5),
        "f6" => Some(Key::KEY_F6),
        "f7" => Some(Key::KEY_F7),
        "f8" => Some(Key::KEY_F8),
        "f9" => Some(Key::KEY_F9),
        "f10" => Some(Key::KEY_F10),
        "f11" => Some(Key::KEY_F11),
        "f12" => Some(Key::KEY_F12),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple() {
        let combo = parse_accelerator("<Control>b");
        assert!(combo.modifiers.contains(&Key::KEY_LEFTCTRL));
        assert_eq!(combo.key, Some(Key::KEY_B));
    }

    #[test]
    fn test_parse_multiple_modifiers() {
        let combo = parse_accelerator("<Control><Alt>k");
        assert!(combo.modifiers.contains(&Key::KEY_LEFTCTRL));
        assert!(combo.modifiers.contains(&Key::KEY_LEFTALT));
        assert_eq!(combo.key, Some(Key::KEY_K));
    }

    #[test]
    fn test_parse_no_modifiers() {
        let combo = parse_accelerator("F1");
        assert!(combo.modifiers.is_empty());
        assert_eq!(combo.key, Some(Key::KEY_F1));
    }

    #[test]
    fn test_parse_shift() {
        let combo = parse_accelerator("<Shift>a");
        assert!(combo.modifiers.contains(&Key::KEY_LEFTSHIFT));
        assert_eq!(combo.key, Some(Key::KEY_A));
    }
}
