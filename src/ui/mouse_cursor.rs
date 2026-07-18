use crate::input::MouseButton;
use gtk4::prelude::*;
use gtk4::{Box as GtkBox, Label, Orientation};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const MOUSE_DISPLAY_TIMEOUT_MS: u64 = 2000;

#[derive(Debug)]
struct DisplayedButton {
    _button: MouseButton,
    widget: gtk4::Widget,
    last_active: Instant,
    is_held: bool,
}

pub struct MouseCursorWidget {
    container: GtkBox,
    displayed_buttons: HashMap<MouseButton, DisplayedButton>,
    display_duration: Duration,
}

impl MouseCursorWidget {
    pub fn new() -> Self {
        let container = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(4)
            .halign(gtk4::Align::Start)
            .valign(gtk4::Align::Center)
            .build();

        container.add_css_class("mouse-cursor-container");

        Self {
            container,
            displayed_buttons: HashMap::new(),
            display_duration: Duration::from_millis(MOUSE_DISPLAY_TIMEOUT_MS),
        }
    }

    pub fn widget(&self) -> &GtkBox {
        &self.container
    }

    pub fn handle_event(&mut self, event: &crate::input::MouseEvent) {
        match event {
            crate::input::MouseEvent::Pressed(button) => self.add_button(*button),
            crate::input::MouseEvent::Released(button) => self.remove_button(*button),
            crate::input::MouseEvent::Scroll { delta } => {
                // Show wheel scroll as a brief button press
                let button = if *delta > 0 {
                    crate::input::MouseButton::WheelUp
                } else {
                    crate::input::MouseButton::WheelDown
                };
                // Add as "held" button, remove_expired will clean it up
                self.add_scroll_button(button);
            }
            _ => {}
        }
    }

    fn add_scroll_button(&mut self, button: MouseButton) {
        // Remove existing if any
        if let Some(existing) = self.displayed_buttons.remove(&button) {
            self.container.remove(&existing.widget);
        }

        let widget = self.create_button_widget(button);
        self.container.append(&widget);

        let displayed = DisplayedButton {
            _button: button,
            widget: widget.upcast(),
            last_active: Instant::now(),
            is_held: false, // Not held, will be removed by remove_expired quickly
        };

        self.displayed_buttons.insert(button, displayed);
    }

    fn add_button(&mut self, button: MouseButton) {
        if let Some(existing) = self.displayed_buttons.get_mut(&button) {
            existing.last_active = Instant::now();
            existing.is_held = true;
            existing.widget.remove_css_class("fading");
            return;
        }

        let widget = self.create_button_widget(button);
        self.container.append(&widget);

        let displayed = DisplayedButton {
            _button: button,
            widget: widget.upcast(),
            last_active: Instant::now(),
            is_held: true,
        };

        self.displayed_buttons.insert(button, displayed);
    }

    fn remove_button(&mut self, button: MouseButton) {
        if let Some(displayed) = self.displayed_buttons.get_mut(&button) {
            displayed.is_held = false;
            displayed.last_active = Instant::now();
            displayed.widget.add_css_class("fading");
        }
    }

    fn create_button_widget(&self, button: MouseButton) -> GtkBox {
        let button_box = GtkBox::builder()
            .orientation(Orientation::Horizontal)
            .spacing(0)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .build();
        button_box.add_css_class("mouse-button-bubble");

        let symbol = match button {
            MouseButton::Left => "L",
            MouseButton::Right => "R",
            MouseButton::Middle => "M",
            MouseButton::Side => "4",
            MouseButton::Extra => "5",
            MouseButton::WheelUp => "⟰",
            MouseButton::WheelDown => "⟱",
        };

        let label = Label::builder()
            .label(symbol)
            .halign(gtk4::Align::Center)
            .valign(gtk4::Align::Center)
            .hexpand(true)
            .build();
        button_box.append(&label);

        button_box
    }

    pub fn remove_expired(&mut self) {
        let now = Instant::now();
        let display_duration = self.display_duration;

        let expired: Vec<MouseButton> = self
            .displayed_buttons
            .iter()
            .filter(|(_, d)| !d.is_held && now.duration_since(d.last_active) > display_duration)
            .map(|(b, _)| *b)
            .collect();

        for button in expired {
            if let Some(removed) = self.displayed_buttons.remove(&button) {
                self.container.remove(&removed.widget);
            }
        }
    }

    pub fn has_buttons(&self) -> bool {
        !self.displayed_buttons.is_empty()
    }
}
