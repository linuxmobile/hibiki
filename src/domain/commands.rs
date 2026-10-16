use super::state::{CaptureState, FocusState};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    ActivateKeystrokeMode,
    ActivateBubbleMode,
    ToggleMode,
    TogglePause,
    ToggleFocus,
    SetCaptureState(CaptureState),
    SetFocusState(FocusState),
}
