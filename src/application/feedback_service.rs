use crate::domain::config::DisplayMode;
use crate::domain::{CaptureState, FocusState};
use crate::presentation::overlay_manager::OverlayManager;
use gtk4::Application;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;

pub struct FeedbackService {
    overlay_manager: Rc<RefCell<OverlayManager>>,
    last_capture_state: Cell<CaptureState>,
}

impl FeedbackService {
    pub fn new(app: &Application) -> Self {
        Self {
            overlay_manager: Rc::new(RefCell::new(OverlayManager::new(app))),
            last_capture_state: Cell::new(CaptureState::Active),
        }
    }

    pub fn handle_state_change(&self, capture: CaptureState, focus: FocusState) {
        let last = self.last_capture_state.get();
        self.last_capture_state.set(capture);

        let text = if last == CaptureState::Paused && capture == CaptureState::Active {
            "Resumed"
        } else {
            match capture {
                CaptureState::Paused => "Paused",
                CaptureState::Active => match focus {
                    FocusState::Focused => "Focused",
                    FocusState::Unfocused => "Unfocused",
                },
            }
        };

        self.overlay_manager.borrow_mut().show_feedback(text);
    }

    pub fn handle_mode_change(&self, mode: DisplayMode) {
        let text = match mode {
            DisplayMode::Keystroke => "Keystroke Mode",
            DisplayMode::Bubble => "Bubble Mode",
        };

        self.overlay_manager.borrow_mut().show_feedback(text);
    }
}
