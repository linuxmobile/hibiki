use evdev::Key;
use std::collections::HashMap;
use std::sync::LazyLock;
use xkbcommon::xkb;

const EVDEV_OFFSET: u32 = 8;

static LAYOUT_NAME_MAP: LazyLock<HashMap<&'static str, (&'static str, &'static str)>> =
    LazyLock::new(|| {
        let mut m = HashMap::new();

        m.insert("Spanish (Latin American)", ("latam", ""));
        m.insert("Spanish", ("es", ""));
        m.insert("Spanish (Spain)", ("es", ""));
        m.insert("Spanish (Dvorak)", ("es", "dvorak"));
        m.insert("Spanish (Catalan)", ("es", "cat"));
        m.insert("Spanish (Mexico)", ("latam", ""));
        m.insert("Spanish (Argentina)", ("latam", ""));

        m.insert("English (US)", ("us", ""));
        m.insert("English", ("us", ""));
        m.insert("US", ("us", ""));
        m.insert("English (UK)", ("gb", ""));
        m.insert("English (British)", ("gb", ""));
        m.insert("British", ("gb", ""));
        m.insert("English (Dvorak)", ("us", "dvorak"));
        m.insert("English (Colemak)", ("us", "colemak"));
        m.insert("English (intl)", ("us", "intl"));
        m.insert("English (US, intl.)", ("us", "intl"));
        m.insert("English (US, international)", ("us", "intl"));
        m.insert("English (US, alt. intl.)", ("us", "alt-intl"));
        m.insert("English (Macintosh)", ("us", "mac"));

        m.insert("German", ("de", ""));
        m.insert("German (Switzerland)", ("ch", "de"));
        m.insert("German (Austria)", ("at", ""));
        m.insert("German (Dvorak)", ("de", "dvorak"));
        m.insert("German (Neo)", ("de", "neo"));

        m.insert("French", ("fr", ""));
        m.insert("French (Canada)", ("ca", "fr"));
        m.insert("French (Belgium)", ("be", ""));
        m.insert("French (Switzerland)", ("ch", "fr"));
        m.insert("French (AZERTY)", ("fr", ""));
        m.insert("French (Dvorak)", ("fr", "dvorak"));
        m.insert("French (BEPO)", ("fr", "bepo"));

        m.insert("Portuguese", ("pt", ""));
        m.insert("Portuguese (Brazil)", ("br", ""));
        m.insert("Portuguese (Portugal)", ("pt", ""));
        m.insert("Brazilian", ("br", ""));

        m.insert("Italian", ("it", ""));
        m.insert("Italian (Macintosh)", ("it", "mac"));

        m.insert("Swedish", ("se", ""));
        m.insert("Norwegian", ("no", ""));
        m.insert("Danish", ("dk", ""));
        m.insert("Finnish", ("fi", ""));
        m.insert("Icelandic", ("is", ""));

        m.insert("Polish", ("pl", ""));
        m.insert("Czech", ("cz", ""));
        m.insert("Slovak", ("sk", ""));
        m.insert("Hungarian", ("hu", ""));
        m.insert("Romanian", ("ro", ""));
        m.insert("Croatian", ("hr", ""));
        m.insert("Serbian", ("rs", ""));
        m.insert("Serbian (Latin)", ("rs", "latin"));
        m.insert("Slovenian", ("si", ""));
        m.insert("Bulgarian", ("bg", ""));
        m.insert("Bulgarian (phonetic)", ("bg", "phonetic"));

        m.insert("Russian", ("ru", ""));
        m.insert("Russian (phonetic)", ("ru", "phonetic"));
        m.insert("Ukrainian", ("ua", ""));
        m.insert("Belarusian", ("by", ""));

        m.insert("Greek", ("gr", ""));

        m.insert("Turkish", ("tr", ""));
        m.insert("Turkish (F)", ("tr", "f"));

        m.insert("Arabic", ("ara", ""));

        m.insert("Hebrew", ("il", ""));

        m.insert("Japanese", ("jp", ""));
        m.insert("Japanese (Kana)", ("jp", "kana"));
        m.insert("Korean", ("kr", ""));
        m.insert("Chinese", ("cn", ""));
        m.insert("Thai", ("th", ""));
        m.insert("Vietnamese", ("vn", ""));

        m.insert("Hindi", ("in", ""));
        m.insert("Indian", ("in", ""));

        m.insert("Dutch", ("nl", ""));
        m.insert("Dutch (Belgium)", ("be", ""));

        m.insert("Esperanto", ("epo", ""));
        m.insert("Irish", ("ie", ""));
        m.insert("Estonian", ("ee", ""));
        m.insert("Latvian", ("lv", ""));
        m.insert("Lithuanian", ("lt", ""));

        m
    });

fn parse_layout_name(name: &str) -> (&str, &str) {
    if let Some(&(layout, variant)) = LAYOUT_NAME_MAP.get(name) {
        return (layout, variant);
    }

    for (key, value) in LAYOUT_NAME_MAP.iter() {
        if key.eq_ignore_ascii_case(name) {
            return *value;
        }
    }

    if let Some(paren_pos) = name.find('(') {
        let base = name[..paren_pos].trim().to_lowercase();

        match base.as_str() {
            "spanish" => return ("es", ""),
            "english" => return ("us", ""),
            "german" => return ("de", ""),
            "french" => return ("fr", ""),
            "portuguese" => return ("pt", ""),
            "italian" => return ("it", ""),
            "russian" => return ("ru", ""),
            "polish" => return ("pl", ""),
            "dutch" => return ("nl", ""),
            "swedish" => return ("se", ""),
            "norwegian" => return ("no", ""),
            "danish" => return ("dk", ""),
            "finnish" => return ("fi", ""),
            "japanese" => return ("jp", ""),
            "korean" => return ("kr", ""),
            "chinese" => return ("cn", ""),
            _ => {}
        }
    }

    let trimmed = name.trim();
    if trimmed.len() <= 5 && trimmed.chars().all(|c| c.is_ascii_lowercase()) {
        return (trimmed, "");
    }

    tracing::debug!("Unknown layout '{}', falling back to US English", name);
    ("us", "")
}

pub struct XkbState {
    context: xkb::Context,
    keymap: xkb::Keymap,
    state: xkb::State,
    layout_name: String,
}

impl XkbState {
    pub fn new() -> Option<Self> {
        Self::from_layout_name(None)
    }

    pub fn from_layout_name(name: Option<&str>) -> Option<Self> {
        let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);

        let (layout, variant) = name.map_or(("", ""), parse_layout_name);

        let layout_name = name.unwrap_or("default").to_string();

        tracing::debug!(
            "Creating XKB state: name='{}' -> layout='{}', variant='{}'",
            layout_name,
            layout,
            variant
        );

        let keymap = xkb::Keymap::new_from_names(
            &context,
            "",
            "",
            layout,
            variant,
            None,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        );

        let keymap = match keymap {
            Some(k) => k,
            None => {
                tracing::warn!(
                    "Failed to create keymap for layout '{}', trying default",
                    layout
                );

                xkb::Keymap::new_from_names(
                    &context,
                    "",
                    "",
                    "",
                    "",
                    None,
                    xkb::KEYMAP_COMPILE_NO_FLAGS,
                )?
            }
        };

        let state = xkb::State::new(&keymap);

        Some(Self {
            context,
            keymap,
            state,
            layout_name,
        })
    }

    #[allow(dead_code)]
    pub fn layout_name(&self) -> &str {
        &self.layout_name
    }

    pub fn set_layout(&mut self, name: &str) -> bool {
        let (layout, variant) = parse_layout_name(name);

        tracing::debug!(
            "Switching XKB layout: '{}' -> layout='{}', variant='{}'",
            name,
            layout,
            variant
        );

        let keymap = match xkb::Keymap::new_from_names(
            &self.context,
            "",
            "",
            layout,
            variant,
            None,
            xkb::KEYMAP_COMPILE_NO_FLAGS,
        ) {
            Some(k) => k,
            None => {
                tracing::warn!("Failed to switch to layout '{}' ({})", name, layout);
                return false;
            }
        };

        self.keymap = keymap;
        self.state = xkb::State::new(&self.keymap);
        self.layout_name = name.to_string();
        true
    }

    fn key_to_keycode(key: Key) -> xkb::Keycode {
        let evdev_code = key.code() as u32;
        xkb::Keycode::new(evdev_code + EVDEV_OFFSET)
    }

    pub fn update_key(&mut self, key: Key, pressed: bool) {
        let keycode = Self::key_to_keycode(key);
        let direction = if pressed {
            xkb::KeyDirection::Down
        } else {
            xkb::KeyDirection::Up
        };
        self.state.update_key(keycode, direction);
    }

    #[allow(dead_code)]
    pub fn reset_modifiers(&mut self) {
        let locked = self.state.serialize_mods(xkb::STATE_MODS_LOCKED);

        self.state.update_mask(0, 0, locked, 0, 0, 0);
    }

    pub fn key_get_utf8(&self, key: Key) -> Option<String> {
        let utf8 = xkb::keysym_to_utf8(self.key_get_one_sym(key));

        if utf8.is_empty() {
            None
        } else {
            Some(utf8)
        }
    }

    pub fn key_get_one_sym(&self, key: Key) -> xkb::Keysym {
        let keycode = Self::key_to_keycode(key);
        self.state.key_get_one_sym(keycode)
    }

    #[allow(dead_code)]
    pub fn mod_name_is_active(&self, name: &str) -> bool {
        self.state
            .mod_name_is_active(name, xkb::STATE_MODS_EFFECTIVE)
    }

    #[allow(dead_code)]
    pub fn is_shift_active(&self) -> bool {
        self.mod_name_is_active(xkb::MOD_NAME_SHIFT)
    }

    #[allow(dead_code)]
    pub fn is_ctrl_active(&self) -> bool {
        self.mod_name_is_active(xkb::MOD_NAME_CTRL)
    }

    #[allow(dead_code)]
    pub fn is_alt_active(&self) -> bool {
        self.mod_name_is_active(xkb::MOD_NAME_ALT)
    }

    #[allow(dead_code)]
    pub fn is_super_active(&self) -> bool {
        self.mod_name_is_active(xkb::MOD_NAME_LOGO)
    }
}

impl Default for XkbState {
    #[allow(clippy::expect_used)]
    fn default() -> Self {
        Self::new().unwrap_or_else(|| {
            let context = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);
            let keymap = xkb::Keymap::new_from_names(
                &context,
                "",
                "",
                "",
                "",
                None,
                xkb::KEYMAP_COMPILE_NO_FLAGS,
            )
            .expect("Failed to create fallback XKB keymap - XKB may not be installed");
            let state = xkb::State::new(&keymap);
            Self {
                context,
                keymap,
                state,
                layout_name: "default".to_string(),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_layout_name_direct() {
        assert_eq!(parse_layout_name("Spanish (Latin American)"), ("latam", ""));
        assert_eq!(parse_layout_name("English (US)"), ("us", ""));
        assert_eq!(parse_layout_name("German"), ("de", ""));
        assert_eq!(parse_layout_name("French (Canada)"), ("ca", "fr"));
    }

    #[test]
    fn test_parse_layout_name_fallback() {
        let (layout, _) = parse_layout_name("Unknown Layout XYZ");
        assert_eq!(layout, "us");
    }

    #[test]
    fn test_parse_layout_name_short() {
        assert_eq!(parse_layout_name("us"), ("us", ""));
        assert_eq!(parse_layout_name("de"), ("de", ""));
        assert_eq!(parse_layout_name("latam"), ("latam", ""));
    }

    #[test]
    fn test_xkb_state_creation() {
        let state = XkbState::new();
        assert!(state.is_some());
    }

    #[test]
    fn test_xkb_state_from_layout() {
        let state = XkbState::from_layout_name(Some("Spanish (Latin American)"));
        assert!(state.is_some());
        let state = state.unwrap();
        assert_eq!(state.layout_name(), "Spanish (Latin American)");
    }

    #[test]
    fn test_xkb_key_translation() {
        let state = XkbState::from_layout_name(Some("English (US)")).unwrap();

        let result = state.key_get_utf8(Key::KEY_A);
        assert_eq!(result, Some("a".to_string()));

        let result = state.key_get_utf8(Key::KEY_1);
        assert_eq!(result, Some("1".to_string()));
    }

    #[test]
    fn test_xkb_shift_modifier() {
        let mut state = XkbState::from_layout_name(Some("English (US)")).unwrap();

        assert!(!state.is_shift_active());
        let result = state.key_get_utf8(Key::KEY_A);
        assert_eq!(result, Some("a".to_string()));

        state.update_key(Key::KEY_LEFTSHIFT, true);
        assert!(state.is_shift_active());

        let result = state.key_get_utf8(Key::KEY_A);
        assert_eq!(result, Some("A".to_string()));

        let result = state.key_get_utf8(Key::KEY_2);
        assert_eq!(result, Some("@".to_string()));

        state.update_key(Key::KEY_LEFTSHIFT, false);
        assert!(!state.is_shift_active());
    }

    #[test]
    fn test_xkb_spanish_latam_layout() {
        let mut state = XkbState::from_layout_name(Some("Spanish (Latin American)")).unwrap();

        state.update_key(Key::KEY_LEFTSHIFT, true);

        let result = state.key_get_utf8(Key::KEY_2);
        assert_eq!(result, Some("\"".to_string()));

        let result = state.key_get_utf8(Key::KEY_3);
        assert_eq!(result, Some("#".to_string()));
    }

    #[test]
    fn test_xkb_layout_switch() {
        let mut state = XkbState::from_layout_name(Some("English (US)")).unwrap();

        assert!(state.set_layout("German"));
        assert_eq!(state.layout_name(), "German");

        let result = state.key_get_utf8(Key::KEY_Z);
        assert_eq!(result, Some("y".to_string()));
    }

    #[test]
    fn test_xkb_ctrl_does_not_uppercase_letter() {
        let mut state = XkbState::from_layout_name(Some("English (US)")).unwrap();

        assert_eq!(state.key_get_utf8(Key::KEY_C), Some("c".to_string()));

        state.update_key(Key::KEY_LEFTCTRL, true);

        assert_eq!(state.key_get_utf8(Key::KEY_C), Some("c".to_string()));
    }

    #[test]
    fn test_xkb_reset_modifiers() {
        let mut state = XkbState::from_layout_name(Some("English (US)")).unwrap();

        state.update_key(Key::KEY_LEFTSHIFT, true);
        assert!(state.is_shift_active());

        state.reset_modifiers();
        assert!(!state.is_shift_active());

        let result = state.key_get_utf8(Key::KEY_A);
        assert_eq!(result, Some("a".to_string()));
    }
}
