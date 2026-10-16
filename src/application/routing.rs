use crate::domain::config::DisplayMode;
use crate::domain::{CaptureState, Command, FocusState};
use crate::infrastructure::hotkey::GlobalHotKeyManager;
use evdev::Key;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum RoutingResult {
    Ignored,
    Dispatch(Key, bool),
    StateChanged(CaptureState, FocusState),
    SwitchMode(DisplayMode),
}

pub struct RoutingEngine {
    hotkey_manager: GlobalHotKeyManager,
    capture_state: CaptureState,
    focus_state: FocusState,
}

impl Default for RoutingEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl RoutingEngine {
    pub fn new() -> Self {
        Self {
            hotkey_manager: GlobalHotKeyManager::new(),
            capture_state: CaptureState::Active,
            focus_state: FocusState::Focused,
        }
    }

    pub fn toggle_capture(&mut self) -> CaptureState {
        self.capture_state = match self.capture_state {
            CaptureState::Active => CaptureState::Paused,
            CaptureState::Paused => CaptureState::Active,
        };
        self.capture_state
    }

    pub fn toggle_focus(&mut self) -> FocusState {
        self.focus_state = match self.focus_state {
            FocusState::Focused => FocusState::Unfocused,
            FocusState::Unfocused => FocusState::Focused,
        };
        self.focus_state
    }

    pub fn process(
        &mut self,
        key: Key,
        pressed: bool,
        enable_focus_toggle: bool,
        current_mode: DisplayMode,
    ) -> RoutingResult {
        if let Some(command) = self.hotkey_manager.process(key, pressed) {
            tracing::info!("Hotkey detected: {:?} in mode {:?}", command, current_mode);
            match command {
                Command::ActivateKeystrokeMode => {
                    if current_mode != DisplayMode::Keystroke {
                        return RoutingResult::SwitchMode(DisplayMode::Keystroke);
                    }
                    return RoutingResult::Ignored;
                }
                Command::ActivateBubbleMode => {
                    if current_mode != DisplayMode::Bubble {
                        return RoutingResult::SwitchMode(DisplayMode::Bubble);
                    }
                    return RoutingResult::Ignored;
                }
                Command::ToggleMode => {
                    let target = match current_mode {
                        DisplayMode::Keystroke => DisplayMode::Bubble,
                        DisplayMode::Bubble => DisplayMode::Keystroke,
                    };
                    return RoutingResult::SwitchMode(target);
                }
                Command::TogglePause => {
                    self.capture_state = match self.capture_state {
                        CaptureState::Active => CaptureState::Paused,
                        CaptureState::Paused => CaptureState::Active,
                    };
                    return RoutingResult::StateChanged(self.capture_state, self.focus_state);
                }
                Command::ToggleFocus => {
                    if enable_focus_toggle {
                        self.focus_state = match self.focus_state {
                            FocusState::Focused => FocusState::Unfocused,
                            FocusState::Unfocused => FocusState::Focused,
                        };
                        return RoutingResult::StateChanged(self.capture_state, self.focus_state);
                    }
                    return RoutingResult::Dispatch(key, pressed);
                }
                _ => {}
            }
        }

        if enable_focus_toggle
            && current_mode == DisplayMode::Bubble
            && pressed
            && key == Key::KEY_ESC
            && self.focus_state == FocusState::Focused
        {
            self.focus_state = FocusState::Unfocused;
            return RoutingResult::StateChanged(self.capture_state, self.focus_state);
        }

        if self.capture_state == CaptureState::Paused {
            return RoutingResult::Ignored;
        }

        if self.focus_state == FocusState::Unfocused && current_mode == DisplayMode::Bubble {
            return RoutingResult::Ignored;
        }

        RoutingResult::Dispatch(key, pressed)
    }

    pub fn get_states(&self) -> (CaptureState, FocusState) {
        (self.capture_state, self.focus_state)
    }

    pub fn update_config(
        &mut self,
        capture_hotkey: &str,
        focus_hotkey: &str,
        pause_hotkey: &str,
        toggle_focus_hotkey: &str,
        toggle_mode_hotkey: &str,
    ) {
        self.hotkey_manager.set_capture_hotkey(capture_hotkey);
        self.hotkey_manager.set_focus_hotkey(focus_hotkey);
        self.hotkey_manager.set_pause_hotkey(pause_hotkey);
        self.hotkey_manager
            .set_toggle_focus_hotkey(toggle_focus_hotkey);
        self.hotkey_manager
            .set_toggle_mode_hotkey(toggle_mode_hotkey);
    }

    pub fn reset_hotkey_state(&mut self) {
        self.hotkey_manager.reset();
    }

    pub fn reset_mode_state(&mut self) {
        self.capture_state = CaptureState::Active;
        self.focus_state = FocusState::Focused;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::config::DisplayMode;
    use crate::domain::{CaptureState, FocusState};
    use evdev::Key;

    fn setup_engine() -> RoutingEngine {
        let mut engine = RoutingEngine::new();
        engine.update_config(
            "<Shift><Control>k",
            "<Shift><Control>b",
            "<Control>p",
            "<Control>f",
            "<Shift><Control>t",
        );
        engine
    }

    #[test]
    fn test_initial_state() {
        let engine = RoutingEngine::new();
        let (capture, focus) = engine.get_states();
        assert_eq!(capture, CaptureState::Active);
        assert_eq!(focus, FocusState::Focused);
    }

    #[test]
    fn test_basic_dispatch() {
        let mut engine = setup_engine();
        let result = engine.process(Key::KEY_A, true, true, DisplayMode::Keystroke);
        assert_eq!(result, RoutingResult::Dispatch(Key::KEY_A, true));
    }

    #[test]
    fn test_mode_switch() {
        let mut engine = setup_engine();

        engine.process(Key::KEY_LEFTSHIFT, true, true, DisplayMode::Keystroke);
        engine.process(Key::KEY_LEFTCTRL, true, true, DisplayMode::Keystroke);
        let result = engine.process(Key::KEY_K, true, true, DisplayMode::Keystroke);
        assert_eq!(result, RoutingResult::Ignored);

        engine.reset_hotkey_state();

        engine.process(Key::KEY_LEFTSHIFT, true, true, DisplayMode::Keystroke);
        engine.process(Key::KEY_LEFTCTRL, true, true, DisplayMode::Keystroke);
        let result = engine.process(Key::KEY_B, true, true, DisplayMode::Keystroke);
        assert_eq!(result, RoutingResult::SwitchMode(DisplayMode::Bubble));

        engine.reset_hotkey_state();

        engine.process(Key::KEY_LEFTSHIFT, true, true, DisplayMode::Bubble);
        engine.process(Key::KEY_LEFTCTRL, true, true, DisplayMode::Bubble);
        let result = engine.process(Key::KEY_B, true, true, DisplayMode::Bubble);
        assert_eq!(result, RoutingResult::Ignored);

        engine.reset_hotkey_state();

        engine.process(Key::KEY_LEFTSHIFT, true, true, DisplayMode::Bubble);
        engine.process(Key::KEY_LEFTCTRL, true, true, DisplayMode::Bubble);
        let result = engine.process(Key::KEY_K, true, true, DisplayMode::Bubble);
        assert_eq!(result, RoutingResult::SwitchMode(DisplayMode::Keystroke));
    }

    #[test]
    fn test_toggle_mode_hotkey_switches_modes() {
        let mut engine = setup_engine();

        engine.process(Key::KEY_LEFTSHIFT, true, true, DisplayMode::Keystroke);
        engine.process(Key::KEY_LEFTCTRL, true, true, DisplayMode::Keystroke);
        let result = engine.process(Key::KEY_T, true, true, DisplayMode::Keystroke);
        assert_eq!(result, RoutingResult::SwitchMode(DisplayMode::Bubble));

        engine.reset_hotkey_state();

        engine.process(Key::KEY_LEFTSHIFT, true, true, DisplayMode::Bubble);
        engine.process(Key::KEY_LEFTCTRL, true, true, DisplayMode::Bubble);
        let result = engine.process(Key::KEY_T, true, true, DisplayMode::Bubble);
        assert_eq!(result, RoutingResult::SwitchMode(DisplayMode::Keystroke));
    }

    #[test]
    fn test_toggle_pause() {
        let mut engine = setup_engine();

        engine.process(Key::KEY_LEFTCTRL, true, true, DisplayMode::Keystroke);
        let result = engine.process(Key::KEY_P, true, true, DisplayMode::Keystroke);
        assert_eq!(
            result,
            RoutingResult::StateChanged(CaptureState::Paused, FocusState::Focused)
        );

        let result = engine.process(Key::KEY_A, true, true, DisplayMode::Keystroke);
        assert_eq!(result, RoutingResult::Ignored);

        engine.reset_hotkey_state();

        engine.process(Key::KEY_LEFTCTRL, true, true, DisplayMode::Keystroke);
        let result = engine.process(Key::KEY_P, true, true, DisplayMode::Keystroke);
        assert_eq!(
            result,
            RoutingResult::StateChanged(CaptureState::Active, FocusState::Focused)
        );

        let result = engine.process(Key::KEY_A, true, true, DisplayMode::Keystroke);
        assert_eq!(result, RoutingResult::Dispatch(Key::KEY_A, true));
    }

    #[test]
    fn test_toggle_focus() {
        let mut engine = setup_engine();

        engine.process(Key::KEY_LEFTCTRL, true, true, DisplayMode::Keystroke);
        let result = engine.process(Key::KEY_F, true, true, DisplayMode::Keystroke);
        assert_eq!(
            result,
            RoutingResult::StateChanged(CaptureState::Active, FocusState::Unfocused)
        );

        engine.reset_hotkey_state();

        engine.process(Key::KEY_LEFTCTRL, true, true, DisplayMode::Keystroke);
        let result = engine.process(Key::KEY_F, true, true, DisplayMode::Keystroke);
        assert_eq!(
            result,
            RoutingResult::StateChanged(CaptureState::Active, FocusState::Focused)
        );
    }

    #[test]
    fn test_toggle_focus_disabled() {
        let mut engine = setup_engine();

        engine.process(Key::KEY_LEFTCTRL, true, false, DisplayMode::Keystroke);
        let result = engine.process(Key::KEY_F, true, false, DisplayMode::Keystroke);

        assert_eq!(result, RoutingResult::Dispatch(Key::KEY_F, true));

        let (_, focus) = engine.get_states();
        assert_eq!(focus, FocusState::Focused);
    }

    #[test]
    fn test_escape_loses_focus_in_bubble_mode() {
        let mut engine = setup_engine();

        let result = engine.process(Key::KEY_ESC, true, true, DisplayMode::Bubble);
        assert_eq!(
            result,
            RoutingResult::StateChanged(CaptureState::Active, FocusState::Unfocused)
        );

        let result = engine.process(Key::KEY_ESC, true, true, DisplayMode::Bubble);
        assert_eq!(result, RoutingResult::Ignored);
    }

    #[test]
    fn test_escape_is_dispatched_in_keystroke_mode() {
        let mut engine = setup_engine();
        let result = engine.process(Key::KEY_ESC, true, true, DisplayMode::Keystroke);
        assert_eq!(result, RoutingResult::Dispatch(Key::KEY_ESC, true));
    }

    #[test]
    fn test_bubble_mode_unfocused_behavior() {
        let mut engine = setup_engine();

        engine.toggle_focus();

        let result = engine.process(Key::KEY_A, true, true, DisplayMode::Bubble);
        assert_eq!(result, RoutingResult::Ignored);

        let result = engine.process(Key::KEY_A, true, true, DisplayMode::Keystroke);
        assert_eq!(result, RoutingResult::Dispatch(Key::KEY_A, true));
    }

    #[test]
    fn test_reset_state() {
        let mut engine = setup_engine();

        engine.toggle_capture();
        engine.toggle_focus();
        assert_eq!(
            engine.get_states(),
            (CaptureState::Paused, FocusState::Unfocused)
        );

        engine.reset_mode_state();
        assert_eq!(
            engine.get_states(),
            (CaptureState::Active, FocusState::Focused)
        );
    }
}
