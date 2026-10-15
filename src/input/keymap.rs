use evdev::Key;
use std::collections::HashMap;
use std::sync::LazyLock;

static KEY_NAMES: LazyLock<HashMap<Key, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    m.insert(Key::KEY_LEFTCTRL, "󰘴");
    m.insert(Key::KEY_RIGHTCTRL, "󰘴");
    m.insert(Key::KEY_LEFTSHIFT, "󰘶");
    m.insert(Key::KEY_RIGHTSHIFT, "󰘶");
    m.insert(Key::KEY_LEFTALT, "󰘵");
    m.insert(Key::KEY_RIGHTALT, "󰘵");
    m.insert(Key::KEY_LEFTMETA, "󰖳");
    m.insert(Key::KEY_RIGHTMETA, "󰖳");
    m.insert(Key::KEY_CAPSLOCK, "󰪛");

    m.insert(Key::KEY_F1, "F1");
    m.insert(Key::KEY_F2, "F2");
    m.insert(Key::KEY_F3, "F3");
    m.insert(Key::KEY_F4, "F4");
    m.insert(Key::KEY_F5, "F5");
    m.insert(Key::KEY_F6, "F6");
    m.insert(Key::KEY_F7, "F7");
    m.insert(Key::KEY_F8, "F8");
    m.insert(Key::KEY_F9, "F9");
    m.insert(Key::KEY_F10, "F10");
    m.insert(Key::KEY_F11, "F11");
    m.insert(Key::KEY_F12, "F12");

    m.insert(Key::KEY_ESC, "󱊷");
    m.insert(Key::KEY_TAB, "󰌒");
    m.insert(Key::KEY_BACKSPACE, "󰭜");
    m.insert(Key::KEY_ENTER, "󰌑");
    m.insert(Key::KEY_SPACE, "󱁐");
    m.insert(Key::KEY_INSERT, "󰎂");
    m.insert(Key::KEY_DELETE, "󰹾");
    m.insert(Key::KEY_HOME, "󰋜");
    m.insert(Key::KEY_END, "󰟀");
    m.insert(Key::KEY_PAGEUP, "󰞕");
    m.insert(Key::KEY_PAGEDOWN, "󰞒");
    m.insert(Key::KEY_UP, "󰁝");
    m.insert(Key::KEY_DOWN, "󰁅");
    m.insert(Key::KEY_LEFT, "󰁍");
    m.insert(Key::KEY_RIGHT, "󰁔");

    m.insert(Key::KEY_0, "0");
    m.insert(Key::KEY_1, "1");
    m.insert(Key::KEY_2, "2");
    m.insert(Key::KEY_3, "3");
    m.insert(Key::KEY_4, "4");
    m.insert(Key::KEY_5, "5");
    m.insert(Key::KEY_6, "6");
    m.insert(Key::KEY_7, "7");
    m.insert(Key::KEY_8, "8");
    m.insert(Key::KEY_9, "9");

    m.insert(Key::KEY_A, "A");
    m.insert(Key::KEY_B, "B");
    m.insert(Key::KEY_C, "C");
    m.insert(Key::KEY_D, "D");
    m.insert(Key::KEY_E, "E");
    m.insert(Key::KEY_F, "F");
    m.insert(Key::KEY_G, "G");
    m.insert(Key::KEY_H, "H");
    m.insert(Key::KEY_I, "I");
    m.insert(Key::KEY_J, "J");
    m.insert(Key::KEY_K, "K");
    m.insert(Key::KEY_L, "L");
    m.insert(Key::KEY_M, "M");
    m.insert(Key::KEY_N, "N");
    m.insert(Key::KEY_O, "O");
    m.insert(Key::KEY_P, "P");
    m.insert(Key::KEY_Q, "Q");
    m.insert(Key::KEY_R, "R");
    m.insert(Key::KEY_S, "S");
    m.insert(Key::KEY_T, "T");
    m.insert(Key::KEY_U, "U");
    m.insert(Key::KEY_V, "V");
    m.insert(Key::KEY_W, "W");
    m.insert(Key::KEY_X, "X");
    m.insert(Key::KEY_Y, "Y");
    m.insert(Key::KEY_Z, "Z");

    m.insert(Key::KEY_MINUS, "-");
    m.insert(Key::KEY_EQUAL, "=");
    m.insert(Key::KEY_LEFTBRACE, "[");
    m.insert(Key::KEY_RIGHTBRACE, "]");
    m.insert(Key::KEY_SEMICOLON, ";");
    m.insert(Key::KEY_APOSTROPHE, "'");
    m.insert(Key::KEY_GRAVE, "`");
    m.insert(Key::KEY_BACKSLASH, "\\");
    m.insert(Key::KEY_COMMA, ",");
    m.insert(Key::KEY_DOT, ".");
    m.insert(Key::KEY_SLASH, "/");

    m.insert(Key::KEY_NUMLOCK, "NumLock");
    m.insert(Key::KEY_KP0, "Num0");
    m.insert(Key::KEY_KP1, "Num1");
    m.insert(Key::KEY_KP2, "Num2");
    m.insert(Key::KEY_KP3, "Num3");
    m.insert(Key::KEY_KP4, "Num4");
    m.insert(Key::KEY_KP5, "Num5");
    m.insert(Key::KEY_KP6, "Num6");
    m.insert(Key::KEY_KP7, "Num7");
    m.insert(Key::KEY_KP8, "Num8");
    m.insert(Key::KEY_KP9, "Num9");
    m.insert(Key::KEY_KPPLUS, "Num+");
    m.insert(Key::KEY_KPMINUS, "Num-");
    m.insert(Key::KEY_KPASTERISK, "Num*");
    m.insert(Key::KEY_KPSLASH, "Num/");
    m.insert(Key::KEY_KPDOT, "Num.");
    m.insert(Key::KEY_KPENTER, "NumEnter");

    m.insert(Key::KEY_MUTE, "Mute");
    m.insert(Key::KEY_VOLUMEDOWN, "Vol-");
    m.insert(Key::KEY_VOLUMEUP, "Vol+");
    m.insert(Key::KEY_PLAYPAUSE, "Play/Pause");
    m.insert(Key::KEY_STOPCD, "Stop");
    m.insert(Key::KEY_PREVIOUSSONG, "Prev");
    m.insert(Key::KEY_NEXTSONG, "Next");

    m.insert(Key::KEY_PRINT, "Print");
    m.insert(Key::KEY_SCROLLLOCK, "ScrollLock");
    m.insert(Key::KEY_PAUSE, "Pause");
    m.insert(Key::KEY_SYSRQ, "SysRq");

    m
});

#[derive(Debug, Clone)]
pub struct KeyDisplay {
    pub key: Key,

    #[allow(dead_code)]
    pub display_name: String,

    #[allow(dead_code)]
    pub pressed: bool,

    pub is_repeat: bool,
}

impl KeyDisplay {
    pub fn new(key: Key, pressed: bool) -> Self {
        let display_name = key_to_display_name(key);
        Self {
            key,
            display_name,
            pressed,
            is_repeat: false,
        }
    }

    pub fn new_repeat(key: Key) -> Self {
        let display_name = key_to_display_name(key);
        Self {
            key,
            display_name,
            pressed: true,
            is_repeat: true,
        }
    }
}

pub fn key_to_display_name(key: Key) -> String {
    KEY_NAMES
        .get(&key)
        .map(|s| s.to_string())
        .unwrap_or_else(|| format!("{:?}", key).replace("KEY_", ""))
}

pub fn is_modifier(key: Key) -> bool {
    matches!(
        key,
        Key::KEY_LEFTCTRL
            | Key::KEY_RIGHTCTRL
            | Key::KEY_LEFTSHIFT
            | Key::KEY_RIGHTSHIFT
            | Key::KEY_LEFTALT
            | Key::KEY_RIGHTALT
            | Key::KEY_LEFTMETA
            | Key::KEY_RIGHTMETA
    )
}

pub fn is_super_key(key: Key) -> bool {
    matches!(key, Key::KEY_LEFTMETA | Key::KEY_RIGHTMETA)
}

/// Whether a key's printed label should come from the active keyboard layout.
///
/// Letters, digits and punctuation are translated through XKB so the visualizer
/// reflects the layout the user is actually typing on. Keys with a fixed
/// emblem (modifiers, function keys, arrows, keypad, media keys, ...) keep
/// their symbolic name, and unmapped keys are translated whenever XKB can.
#[must_use]
pub fn is_layout_key(key: Key) -> bool {
    match KEY_NAMES.get(&key) {
        Some(name) => {
            let mut chars = name.chars();
            matches!((chars.next(), chars.next()), (Some(c), None) if c.is_ascii())
        }
        None => true,
    }
}

#[allow(dead_code)]
pub fn normalize_modifier(key: Key) -> Key {
    match key {
        Key::KEY_RIGHTCTRL => Key::KEY_LEFTCTRL,
        Key::KEY_RIGHTSHIFT => Key::KEY_LEFTSHIFT,
        Key::KEY_RIGHTALT => Key::KEY_LEFTALT,
        Key::KEY_RIGHTMETA => Key::KEY_LEFTMETA,
        _ => key,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_to_display_name() {
        assert_eq!(key_to_display_name(Key::KEY_A), "A");
        assert_eq!(key_to_display_name(Key::KEY_LEFTCTRL), "󰘴");
        assert_eq!(key_to_display_name(Key::KEY_SPACE), "󱁐");
    }

    #[test]
    fn test_is_modifier() {
        assert!(is_modifier(Key::KEY_LEFTCTRL));
        assert!(is_modifier(Key::KEY_RIGHTSHIFT));
        assert!(!is_modifier(Key::KEY_A));
    }

    #[test]
    fn test_is_layout_key() {
        assert!(is_layout_key(Key::KEY_A));
        assert!(is_layout_key(Key::KEY_Z));
        assert!(is_layout_key(Key::KEY_1));
        assert!(is_layout_key(Key::KEY_SEMICOLON));
        assert!(is_layout_key(Key::KEY_102ND));

        assert!(!is_layout_key(Key::KEY_LEFTCTRL));
        assert!(!is_layout_key(Key::KEY_LEFTSHIFT));
        assert!(!is_layout_key(Key::KEY_F1));
        assert!(!is_layout_key(Key::KEY_SPACE));
        assert!(!is_layout_key(Key::KEY_KP0));
    }

    #[test]
    fn test_normalize_modifier() {
        assert_eq!(normalize_modifier(Key::KEY_RIGHTCTRL), Key::KEY_LEFTCTRL);
        assert_eq!(normalize_modifier(Key::KEY_A), Key::KEY_A);
    }
}

pub fn map_evdev_to_mechvibes(key: u16) -> Option<u32> {
    match Key::new(key) {
        Key::KEY_UP => Some(57416),
        Key::KEY_LEFT => Some(57419),
        Key::KEY_RIGHT => Some(57421),
        Key::KEY_DOWN => Some(57424),

        Key::KEY_INSERT => Some(57426),
        Key::KEY_DELETE => Some(57427),
        Key::KEY_HOME => Some(57415),
        Key::KEY_END => Some(57423),
        Key::KEY_PAGEUP => Some(57417),
        Key::KEY_PAGEDOWN => Some(57425),

        Key::KEY_RIGHTALT => Some(57400),
        Key::KEY_RIGHTCTRL => Some(57373),
        Key::KEY_RIGHTMETA => Some(57435),
        Key::KEY_LEFTMETA => Some(57435),

        Key::KEY_KPSLASH => Some(57397),
        Key::KEY_KPENTER => Some(57372),

        Key::KEY_MUTE => Some(57376),
        Key::KEY_VOLUMEDOWN => Some(57390),
        Key::KEY_VOLUMEUP => Some(57392),
        Key::KEY_PLAYPAUSE => Some(57378),
        Key::KEY_STOPCD => Some(57380),
        Key::KEY_PREVIOUSSONG => Some(57360),
        Key::KEY_NEXTSONG => Some(57369),

        _ => None,
    }
}

#[cfg(test)]
mod additional_tests {
    use super::*;
    use evdev::Key;

    #[test]
    fn test_map_evdev_to_mechvibes() {
        assert_eq!(map_evdev_to_mechvibes(Key::KEY_UP.code()), Some(57416));
        assert_eq!(map_evdev_to_mechvibes(Key::KEY_DOWN.code()), Some(57424));

        assert_eq!(map_evdev_to_mechvibes(Key::KEY_DELETE.code()), Some(57427));
        assert_eq!(map_evdev_to_mechvibes(Key::KEY_HOME.code()), Some(57415));

        assert_eq!(
            map_evdev_to_mechvibes(Key::KEY_RIGHTCTRL.code()),
            Some(57373)
        );

        assert_eq!(map_evdev_to_mechvibes(Key::KEY_A.code()), None);
    }
}
