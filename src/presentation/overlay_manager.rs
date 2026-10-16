use crate::presentation::components::osd_bubble::OsdBubble;
use gtk4::glib;
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow};
use gtk4_layer_shell::{Edge, Layer, LayerShell};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

pub struct OverlayManager {
    window: ApplicationWindow,
    bubble: OsdBubble,
    active_source: Rc<RefCell<Option<glib::SourceId>>>,
}

impl OverlayManager {
    pub fn new(app: &Application) -> Self {
        let window = ApplicationWindow::builder()
            .application(app)
            .decorated(false)
            .resizable(false)
            .build();

        let provider = gtk4::CssProvider::new();
        provider.load_from_string(include_str!("../../style/osd.css"));

        if let Some(display) = gtk4::gdk::Display::default() {
            gtk4::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
            );
        }

        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_namespace("hibiki-osd");
        window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::None);

        window.set_anchor(Edge::Top, false);
        window.set_anchor(Edge::Bottom, true);
        window.set_anchor(Edge::Left, false);
        window.set_anchor(Edge::Right, false);
        window.set_exclusive_zone(0);

        window.add_css_class("osd-window");

        let bubble = OsdBubble::new();
        window.set_child(Some(bubble.widget()));

        Self {
            window,
            bubble,
            active_source: Rc::new(RefCell::new(None)),
        }
    }

    pub fn show_feedback(&mut self, text: &str) {
        self.bubble.set_text(text);

        self.window.set_visible(true);

        if let Some(source) = self.active_source.borrow_mut().take() {
            source.remove();
        }

        let window = self.window.clone();
        let active_source_clone = self.active_source.clone();

        let source_id = glib::timeout_add_local(Duration::from_millis(1500), move || {
            window.set_visible(false);
            *active_source_clone.borrow_mut() = None;
            glib::ControlFlow::Break
        });

        *self.active_source.borrow_mut() = Some(source_id);
    }
}
