use crate::application::audio_dispatcher::{AudioDispatcher, AudioEngine};
use crate::application::config_service::ConfigService;
use crate::application::feedback_service::FeedbackService;
use crate::application::routing::{RoutingEngine, RoutingResult};
use crate::application::typography_service::TypographyService;
use crate::compositor::LayoutEvent;
use crate::domain::FocusState;
use crate::domain::{CaptureState, Command, GlobalShortcutsBackend, KeystrokeConfig};
use crate::infrastructure::portal_shortcuts::{self, PortalHandle, Shortcut};
use crate::input::{
    InputControlCommand, KeyEvent, KeyListener, LayoutManager, ListenerConfig, ListenerHandle,
};
use crate::tray::{AppAction, TrayHandle};
use crate::ui::{
    alignment_for, create_bubble_window, create_launcher_window, create_settings_window,
    create_window, setup_drag, show_launcher, show_settings, BubbleDisplayWidget, DisplayMode,
    KeyDisplayWidget,
};
use anyhow::Result;
use async_channel::{bounded, Receiver};
use gtk4::gio;
use gtk4::glib::{self, ControlFlow};
use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow, CssProvider};
use parking_lot::Mutex;
use std::cell::Cell;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

pub struct App {
    gtk_app: Application,
    config_service: ConfigService,
    no_ui: bool,
    #[allow(dead_code)]
    audio_engine: Arc<Mutex<Option<AudioEngine>>>,
    audio_dispatcher: Option<AudioDispatcher>,
    state_slot: Rc<RefCell<Option<Rc<RefCell<RuntimeState>>>>>,
    tray_slot: Rc<RefCell<Option<TrayHandle>>>,
}

#[derive(Default)]
struct RuntimeState {
    mode: Option<DisplayMode>,
    routing_engine: RoutingEngine,
    keystroke_window: Option<ApplicationWindow>,
    bubble_window: Option<ApplicationWindow>,
    launcher_window: Option<ApplicationWindow>,
    settings_window: Option<ApplicationWindow>,
    mode_active: Option<Arc<AtomicBool>>,
    visual_enabled: Option<Rc<Cell<bool>>>,
    listener_handle: Option<ListenerHandle>,
    layout_manager: Option<LayoutManager>,
    feedback_service: Option<FeedbackService>,
    drag_controllers: Vec<gtk4::EventController>,
    css_provider: Option<CssProvider>,
    audio_dispatcher: Option<AudioDispatcher>,
    audio_engine: Option<AudioEngine>,
    portal_handle: Option<PortalHandle>,
    use_portal: bool,
}

impl App {
    #[must_use]
    pub fn new(config_service: ConfigService, no_ui: bool) -> Self {
        let gtk_app = Application::builder()
            .application_id("dev.linuxmobile.hibiki")
            .flags(gio::ApplicationFlags::HANDLES_COMMAND_LINE)
            .build();

        register_cli_options(&gtk_app);

        let (audio_engine, audio_dispatcher) = match AudioEngine::new() {
            Ok((e, d)) => (Some(e), Some(d)),
            Err(e) => {
                warn!("Failed to initialize audio engine: {}", e);
                (None, None)
            }
        };

        let state_slot: Rc<RefCell<Option<Rc<RefCell<RuntimeState>>>>> =
            Rc::new(RefCell::new(None));
        let tray_slot: Rc<RefCell<Option<TrayHandle>>> = Rc::new(RefCell::new(None));

        let command_service = config_service.clone();
        let command_state = Rc::clone(&state_slot);
        let command_tray = Rc::clone(&tray_slot);
        gtk_app.connect_command_line(move |app, cmdline| {
            handle_command_line(
                app,
                cmdline,
                &command_state,
                &command_tray,
                &command_service,
                no_ui,
            )
        });

        Self {
            gtk_app,
            config_service,
            no_ui,
            audio_engine: Arc::new(Mutex::new(audio_engine)),
            audio_dispatcher,
            state_slot,
            tray_slot,
        }
    }

    #[must_use]
    pub fn is_remote(&self) -> bool {
        if let Err(e) = self.gtk_app.register(gtk4::gio::Cancellable::NONE) {
            debug!("Could not register application with the session bus: {}", e);
        }
        self.gtk_app.is_remote()
    }

    #[must_use]
    pub fn run_with_tray(self, tray_rx: Receiver<AppAction>, tray_handle: TrayHandle) -> i32 {
        let config_service = self.config_service.clone();
        let audio_dispatcher = self.audio_dispatcher.clone();
        let audio_engine_container = self.audio_engine.clone();
        let no_ui = self.no_ui;
        let state_slot = self.state_slot.clone();

        *self.tray_slot.borrow_mut() = Some(tray_handle.clone());

        let state_slot_activate = Rc::clone(&state_slot);
        self.gtk_app.connect_activate(move |app| {
            let audio_engine = audio_engine_container.lock().take();
            activate(
                app,
                &config_service,
                tray_rx.clone(),
                tray_handle.clone(),
                audio_dispatcher.clone(),
                audio_engine,
                no_ui,
                &state_slot_activate,
            );
        });

        self.gtk_app.run().into()
    }

    #[must_use]
    pub fn run(self) -> i32 {
        let config_service = self.config_service.clone();
        let audio_dispatcher = self.audio_dispatcher.clone();
        let audio_engine_container = self.audio_engine.clone();
        let no_ui = self.no_ui;
        let state_slot = self.state_slot.clone();

        let state_slot_activate = Rc::clone(&state_slot);
        self.gtk_app.connect_activate(move |app| {
            let audio_engine = audio_engine_container.lock().take();
            activate_without_tray(
                app,
                &config_service,
                audio_dispatcher.clone(),
                audio_engine,
                no_ui,
                &state_slot_activate,
            );
        });

        self.gtk_app.run().into()
    }
}

/// Register the CLI flags with `GApplication` so they can be parsed locally and
/// forwarded to an already-running instance.
fn register_cli_options(app: &Application) {
    let none = glib::Char(0);
    let entries: [(&str, glib::Char, &str); 14] = [
        (
            "keystroke-toggle",
            glib::Char::from(b'k'),
            "Switch to keystroke mode",
        ),
        ("keystroke", none, "Switch to keystroke mode"),
        (
            "bubble-toggle",
            glib::Char::from(b'b'),
            "Switch to bubble mode",
        ),
        ("bubble", none, "Switch to bubble mode"),
        (
            "toggle",
            glib::Char::from(b't'),
            "Toggle between keystroke and bubble mode",
        ),
        (
            "pause-toggle",
            glib::Char::from(b'p'),
            "Pause or resume key capture",
        ),
        ("sound-toggle", none, "Enable or disable key sounds"),
        (
            "visual-toggle",
            none,
            "Show or hide the on-screen visualizer",
        ),
        ("settings", none, "Open the settings window"),
        ("launcher", none, "Show the launcher window"),
        ("quit", none, "Quit the running instance"),
        (
            "no-ui",
            glib::Char::from(b'n'),
            "Skip the launcher on startup",
        ),
        (
            "daemon",
            glib::Char::from(b'd'),
            "Run detached in the background",
        ),
        ("verbose", glib::Char::from(b'v'), "Show detailed logs"),
    ];

    for (long_name, short_name, description) in entries {
        app.add_main_option(
            long_name,
            short_name,
            glib::OptionFlags::NONE,
            glib::OptionArg::None,
            description,
            None,
        );
    }
}

fn actions_from_options(cmdline: &gio::ApplicationCommandLine) -> Vec<AppAction> {
    let options = cmdline.options_dict();
    actions_from_flags(|key| options.contains(key))
}

fn actions_from_flags(is_set: impl Fn(&str) -> bool) -> Vec<AppAction> {
    let mut actions = Vec::new();

    if is_set("keystroke-toggle") || is_set("keystroke") {
        actions.push(AppAction::KeystrokeMode);
    }
    if is_set("bubble-toggle") || is_set("bubble") {
        actions.push(AppAction::BubbleMode);
    }
    if is_set("toggle") {
        actions.push(AppAction::ToggleMode);
    }
    if is_set("pause-toggle") {
        actions.push(AppAction::TogglePause);
    }
    if is_set("settings") {
        actions.push(AppAction::OpenSettings);
    }
    if is_set("launcher") {
        actions.push(AppAction::ShowLauncher);
    }
    if is_set("quit") {
        actions.push(AppAction::Quit);
    }
    if is_set("sound-toggle") {
        actions.push(AppAction::ToggleSound);
    }
    if is_set("visual-toggle") {
        actions.push(AppAction::ToggleVisual);
    }

    actions
}

fn handle_command_line(
    app: &Application,
    cmdline: &gio::ApplicationCommandLine,
    state_slot: &Rc<RefCell<Option<Rc<RefCell<RuntimeState>>>>>,
    tray_slot: &Rc<RefCell<Option<TrayHandle>>>,
    config_service: &ConfigService,
    no_ui: bool,
) -> i32 {
    let actions = actions_from_options(cmdline);
    let is_remote = cmdline.is_remote();

    debug!(
        "Command line received: remote={}, actions={:?}",
        is_remote, actions
    );

    if state_slot.borrow().is_none() {
        app.activate();
    }

    let state = state_slot.borrow().clone();
    let Some(state) = state else {
        return 0;
    };

    let tray = tray_slot.borrow().clone();

    if actions.is_empty() {
        if is_remote && !no_ui {
            handle_app_action(
                &AppAction::ShowLauncher,
                &state,
                config_service,
                app,
                tray.as_ref(),
            );
        }
        return 0;
    }

    for action in actions {
        handle_app_action(&action, &state, config_service, app, tray.as_ref());
    }

    0
}

#[allow(clippy::too_many_arguments)]
fn activate_without_tray(
    app: &Application,
    config_service: &ConfigService,
    audio_dispatcher: Option<AudioDispatcher>,
    audio_engine: Option<AudioEngine>,
    no_ui: bool,
    state_slot: &Rc<RefCell<Option<Rc<RefCell<RuntimeState>>>>>,
) {
    info!("Activating keystroke application (no tray)");

    let state = Rc::new(RefCell::new(RuntimeState::default()));
    state.borrow_mut().feedback_service = Some(FeedbackService::new(app));
    state.borrow_mut().audio_dispatcher = audio_dispatcher;
    state.borrow_mut().audio_engine = audio_engine;

    let config = config_service.get_config();
    apply_hotkey_config(&state, &config);

    let css_provider = load_css_defaults();

    crate::ui::update_css_provider(&css_provider, &config);
    state.borrow_mut().css_provider = Some(css_provider);

    setup_launcher_and_modes(app, &state, config_service.clone(), no_ui);

    setup_portal(app, &state, config_service, None);

    *state_slot.borrow_mut() = Some(Rc::clone(&state));
}

#[allow(clippy::too_many_arguments)]
fn activate(
    app: &Application,
    config_service: &ConfigService,
    tray_rx: Receiver<AppAction>,
    tray_handle: TrayHandle,
    audio_dispatcher: Option<AudioDispatcher>,
    audio_engine: Option<AudioEngine>,
    no_ui: bool,
    state_slot: &Rc<RefCell<Option<Rc<RefCell<RuntimeState>>>>>,
) {
    info!("Activating keystroke application");

    let state = Rc::new(RefCell::new(RuntimeState::default()));
    state.borrow_mut().feedback_service = Some(FeedbackService::new(app));
    state.borrow_mut().audio_dispatcher = audio_dispatcher;
    state.borrow_mut().audio_engine = audio_engine;

    let config = config_service.get_config();
    apply_hotkey_config(&state, &config);

    let css_provider = load_css_defaults();

    crate::ui::update_css_provider(&css_provider, &config);
    state.borrow_mut().css_provider = Some(css_provider);

    setup_launcher_and_modes(app, &state, config_service.clone(), no_ui);

    setup_portal(app, &state, config_service, Some(tray_handle.clone()));

    setup_tray_handling(
        Rc::clone(&state),
        config_service.clone(),
        app.clone(),
        tray_rx,
        tray_handle,
    );

    *state_slot.borrow_mut() = Some(state);
}

/// With the portal owning the shortcuts, evdev matching is disabled so a combo
/// is never handled twice.
fn apply_hotkey_config(state: &Rc<RefCell<RuntimeState>>, config: &KeystrokeConfig) {
    let mut s = state.borrow_mut();
    if s.use_portal {
        s.routing_engine.update_config("", "", "", "", "");
    } else {
        s.routing_engine.update_config(
            &config.keystroke_hotkey,
            &config.bubble.hotkey,
            &config.pause_hotkey,
            &config.toggle_focus_hotkey,
            &config.toggle_mode_hotkey,
        );
    }
}

/// The exchange runs off the main thread; evdev matching stays active until the
/// shortcuts are actually bound.
fn setup_portal(
    app: &Application,
    state: &Rc<RefCell<RuntimeState>>,
    config_service: &ConfigService,
    tray_handle: Option<TrayHandle>,
) {
    let config = config_service.get_config();

    if config.global_shortcuts == GlobalShortcutsBackend::Evdev {
        return;
    }

    let shortcuts = vec![
        Shortcut {
            id: "keystroke-mode",
            description: "Switch to keystroke mode",
            trigger: portal_shortcuts::accelerator_to_trigger(&config.keystroke_hotkey),
        },
        Shortcut {
            id: "bubble-mode",
            description: "Switch to bubble mode",
            trigger: portal_shortcuts::accelerator_to_trigger(&config.bubble.hotkey),
        },
        Shortcut {
            id: "toggle-mode",
            description: "Toggle keystroke/bubble mode",
            trigger: portal_shortcuts::accelerator_to_trigger(&config.toggle_mode_hotkey),
        },
        Shortcut {
            id: "pause",
            description: "Pause or resume key capture",
            trigger: portal_shortcuts::accelerator_to_trigger(&config.pause_hotkey),
        },
        Shortcut {
            id: "toggle-focus",
            description: "Toggle focus (bubble mode)",
            trigger: portal_shortcuts::accelerator_to_trigger(&config.toggle_focus_hotkey),
        },
    ];

    let (handle, ready_rx, command_rx) = portal_shortcuts::start(shortcuts);
    state.borrow_mut().portal_handle = Some(handle);

    let state_c = Rc::clone(state);
    let app_c = app.clone();
    let service_c = config_service.clone();

    glib::MainContext::default().spawn_local(async move {
        let Ok(Ok(())) = ready_rx.recv().await else {
            if service_c.get_config().global_shortcuts == GlobalShortcutsBackend::Portal {
                // The user asked to rely exclusively on the portal.
                state_c.borrow_mut().use_portal = true;
                apply_hotkey_config(&state_c, &service_c.get_config());
                warn!("GlobalShortcuts portal is unavailable; global hotkeys are inactive");
            } else {
                debug!("Global hotkeys stay on raw device capture (portal unavailable)");
            }
            return;
        };

        info!("Global hotkeys are now owned by the desktop portal");
        state_c.borrow_mut().use_portal = true;
        apply_hotkey_config(&state_c, &service_c.get_config());

        while let Ok(command) = command_rx.recv().await {
            handle_portal_command(&command, &state_c, &service_c, &app_c, tray_handle.as_ref());
        }
    });
}

fn handle_portal_command(
    command: &Command,
    state: &Rc<RefCell<RuntimeState>>,
    config_service: &ConfigService,
    app: &Application,
    tray_handle: Option<&TrayHandle>,
) {
    match command {
        Command::ActivateKeystrokeMode => {
            show_mode_feedback(state, DisplayMode::Keystroke);
            switch_mode(app, state, config_service, DisplayMode::Keystroke);
        }
        Command::ActivateBubbleMode => {
            show_mode_feedback(state, DisplayMode::Bubble);
            switch_mode(app, state, config_service, DisplayMode::Bubble);
        }
        Command::ToggleMode => {
            let target = match state.borrow().mode {
                Some(DisplayMode::Keystroke) => DisplayMode::Bubble,
                _ => DisplayMode::Keystroke,
            };
            show_mode_feedback(state, target);
            switch_mode(app, state, config_service, target);
        }
        Command::TogglePause => {
            handle_app_action(
                &AppAction::TogglePause,
                state,
                config_service,
                app,
                tray_handle,
            );
        }
        Command::ToggleFocus => toggle_focus(state),
        Command::SetCaptureState(_) | Command::SetFocusState(_) => {}
    }
}

fn toggle_focus(state: &Rc<RefCell<RuntimeState>>) {
    if state.borrow().mode != Some(DisplayMode::Bubble) {
        return;
    }

    let (capture, focus) = {
        let mut s = state.borrow_mut();
        let focus = s.routing_engine.toggle_focus();
        (s.routing_engine.get_states().0, focus)
    };

    let settings_open = state.borrow().settings_window.is_some();
    let s = state.borrow();

    if let Some(service) = &s.feedback_service {
        service.handle_state_change(capture, focus);
    }

    if let Some(handle) = &s.listener_handle {
        if capture == CaptureState::Active && focus == FocusState::Focused && !settings_open {
            handle.send_command(InputControlCommand::Grab);
        } else {
            handle.send_command(InputControlCommand::Ungrab);
        }
    }
}

fn setup_launcher_and_modes(
    app: &Application,
    state: &Rc<RefCell<RuntimeState>>,
    config_service: ConfigService,
    no_ui: bool,
) {
    let app_for_mode = app.clone();
    let state_for_mode = Rc::clone(state);
    let service_for_mode = config_service.clone();

    let on_mode_select = move |mode| {
        debug!("Mode selected: {:?}", mode);
        switch_mode(&app_for_mode, &state_for_mode, &service_for_mode, mode);
    };

    let app_for_settings = app.clone();
    let state_for_settings = Rc::clone(state);
    let service_for_settings = config_service.clone();

    let on_settings = move || {
        debug!("Opening settings from launcher");
        open_settings(
            &app_for_settings,
            &state_for_settings,
            service_for_settings.clone(),
        );
    };

    let launcher = create_launcher_window(
        app,
        &config_service.get_config(),
        on_mode_select,
        on_settings,
    );

    state.borrow_mut().launcher_window = Some(launcher.clone());

    if no_ui {
        let mode = config_service.get_config().display_mode;
        switch_mode(app, state, &config_service, mode);
    } else {
        show_launcher(&launcher);
    }
}

fn show_mode_feedback(state: &Rc<RefCell<RuntimeState>>, mode: DisplayMode) {
    if let Some(service) = &state.borrow().feedback_service {
        service.handle_mode_change(mode);
    }
}

fn switch_mode(
    app: &Application,
    state: &Rc<RefCell<RuntimeState>>,
    config_service: &ConfigService,
    mode: DisplayMode,
) {
    debug!("Switching to {:?} mode", mode);
    close_mode_windows(state);

    if let Some(launcher) = &state.borrow().launcher_window {
        launcher.set_visible(false);
    }

    let config = config_service.get_config();
    if let Some(provider) = &state.borrow().css_provider {
        crate::ui::update_css_provider(provider, &config);
    }

    state.borrow_mut().mode = Some(mode);

    match mode {
        DisplayMode::Keystroke => {
            if let Err(e) = start_keystroke_mode(app, config_service, state) {
                error!("Failed to start keystroke mode: {}", e);
            }
        }
        DisplayMode::Bubble => {
            if let Err(e) = start_bubble_mode(app, config_service, state) {
                error!("Failed to start bubble mode: {}", e);
            }
        }
    }
}

fn close_mode_windows(state: &Rc<RefCell<RuntimeState>>) {
    let mut s = state.borrow_mut();

    s.routing_engine.reset_hotkey_state();

    if let Some(active) = s.mode_active.take() {
        active.store(false, Ordering::SeqCst);
    }

    s.listener_handle = None;
    s.layout_manager = None;
    s.visual_enabled = None;

    if let Some(window) = &s.keystroke_window {
        for controller in &s.drag_controllers {
            window.remove_controller(controller);
        }
    }
    s.drag_controllers.clear();

    if let Some(window) = s.keystroke_window.take() {
        window.close();
    }
    if let Some(window) = s.bubble_window.take() {
        window.close();
    }
}

fn setup_tray_handling(
    state: Rc<RefCell<RuntimeState>>,
    config_service: ConfigService,
    app: Application,
    tray_rx: Receiver<AppAction>,
    tray_handle: TrayHandle,
) {
    let tray_handle = Arc::new(tray_handle);

    let config = config_service.get_config();
    update_tray_icon(&tray_handle, &config);

    let mut config_rx = config_service.subscribe();
    let tray_handle_c = Arc::clone(&tray_handle);
    glib::MainContext::default().spawn_local(async move {
        while config_rx.changed().await.is_ok() {
            let config = config_rx.borrow().clone();
            update_tray_icon(&tray_handle_c, &config);
        }
    });

    let tray_handle_t = Arc::clone(&tray_handle);
    let config_service_t = config_service.clone();
    if let Some(settings) = gtk4::Settings::default() {
        settings.connect_notify(Some("gtk-application-prefer-dark-theme"), move |_, _| {
            let config = config_service_t.get_config();
            update_tray_icon(&tray_handle_t, &config);
        });
    }

    let tray_handle_a = Arc::clone(&tray_handle);
    glib::MainContext::default().spawn_local(async move {
        while let Ok(action) = tray_rx.recv().await {
            handle_app_action(
                &action,
                &state,
                &config_service,
                &app,
                Some(tray_handle_a.as_ref()),
            );
        }
        debug!("Tray event loop terminated");
    });
}

fn update_tray_icon(
    tray_handle: &Arc<TrayHandle>,
    config: &crate::domain::config::KeystrokeConfig,
) {
    let is_dark = match config.keystroke_theme.as_str() {
        "dark" => true,
        "light" => false,
        _ => gtk4::Settings::default()
            .map(|s| s.is_gtk_application_prefer_dark_theme())
            .unwrap_or(true),
    };

    let theme = if is_dark {
        crate::ui::IconTheme::Dark
    } else {
        crate::ui::IconTheme::Light
    };

    let icon = crate::ui::render_tray_icon(theme);
    if !icon.is_empty() {
        tray_handle.set_icon(icon);
    }
}

fn handle_app_action(
    action: &AppAction,
    state: &Rc<RefCell<RuntimeState>>,
    config_service: &ConfigService,
    app: &Application,
    tray_handle: Option<&TrayHandle>,
) {
    match action {
        AppAction::ShowLauncher => {
            debug!("Handling ShowLauncher action");
            if let Some(launcher) = &state.borrow().launcher_window {
                show_launcher(launcher);
            }
        }
        AppAction::KeystrokeMode => {
            debug!("Handling KeystrokeMode action");
            switch_mode(app, state, config_service, DisplayMode::Keystroke);
        }
        AppAction::BubbleMode => {
            debug!("Handling BubbleMode action");
            switch_mode(app, state, config_service, DisplayMode::Bubble);
        }
        AppAction::ToggleMode => {
            let target = match state.borrow().mode {
                Some(DisplayMode::Keystroke) => DisplayMode::Bubble,
                _ => DisplayMode::Keystroke,
            };
            debug!("Handling ToggleMode action -> {:?}", target);
            switch_mode(app, state, config_service, target);
        }
        AppAction::OpenSettings => {
            debug!("Handling OpenSettings action");
            if let Some(launcher) = &state.borrow().launcher_window {
                launcher.set_visible(false);
            }
            open_settings(app, state, config_service.clone());
        }
        AppAction::TogglePause => {
            debug!("Handling TogglePause action");
            let mut s = state.borrow_mut();
            let new_state = s.routing_engine.toggle_capture();
            let paused = new_state == CaptureState::Paused;

            if let Some(w) = &s.keystroke_window {
                if paused {
                    w.add_css_class("paused");
                } else {
                    w.remove_css_class("paused");
                }
            }
            if let Some(w) = &s.bubble_window {
                if paused {
                    w.add_css_class("paused");
                } else {
                    w.remove_css_class("paused");
                }
            }
            if let Some(service) = &s.feedback_service {
                service.handle_state_change(new_state, s.routing_engine.get_states().1);
            }
            drop(s);
            if let Some(handle) = tray_handle {
                handle.set_paused(paused);
            }
            info!(
                "Keystroke capture {}",
                if paused { "paused" } else { "resumed" }
            );
        }
        AppAction::ToggleSound => {
            debug!("Handling ToggleSound action");
            let mut cfg = config_service.get_config();
            cfg.sound.enabled = !cfg.sound.enabled;
            let enabled = cfg.sound.enabled;
            let _ = config_service.update_config(cfg);
            info!(
                "Key sounds {}",
                if enabled { "enabled" } else { "disabled" }
            );
        }
        AppAction::ToggleVisual => {
            debug!("Handling ToggleVisual action");
            let mut cfg = config_service.get_config();
            cfg.show_visual = !cfg.show_visual;
            let show = cfg.show_visual;
            let _ = config_service.update_config(cfg);

            if !show {
                let s = state.borrow();
                for window in [s.keystroke_window.as_ref(), s.bubble_window.as_ref()]
                    .into_iter()
                    .flatten()
                {
                    window.set_visible(false);
                }
            }
            info!("Visualizer {}", if show { "shown" } else { "hidden" });
        }
        AppAction::Quit => {
            debug!("Handling Quit action");
            app.quit();
        }
    }
}

fn open_settings(
    app: &Application,
    state: &Rc<RefCell<RuntimeState>>,
    config_service: ConfigService,
) {
    if let Some(ref settings_window) = state.borrow().settings_window {
        show_settings(settings_window);
        return;
    }

    let typography_service = TypographyService::new();
    let settings_window = create_settings_window(app, config_service, typography_service);

    state.borrow_mut().settings_window = Some(settings_window.clone());

    // Bubble mode grabs the keyboard exclusively; release it while the settings
    // window is open so its own keybind capture and text fields keep working.
    release_mode_grab(state);

    let state_clone = Rc::clone(state);
    settings_window.connect_close_request(move |_| {
        state_clone.borrow_mut().settings_window = None;
        restore_mode_grab(&state_clone);
        glib::Propagation::Proceed
    });

    show_settings(&settings_window);
}

fn release_mode_grab(state: &Rc<RefCell<RuntimeState>>) {
    let s = state.borrow();
    if let Some(handle) = &s.listener_handle {
        handle.send_command(InputControlCommand::Ungrab);
    }
}

fn restore_mode_grab(state: &Rc<RefCell<RuntimeState>>) {
    let s = state.borrow();
    if s.mode != Some(DisplayMode::Bubble) {
        return;
    }
    if let Some(handle) = &s.listener_handle {
        let (capture, focus) = s.routing_engine.get_states();
        if capture == CaptureState::Active && focus == FocusState::Focused {
            handle.send_command(InputControlCommand::Grab);
        }
    }
}

/// Returns the initial layout name so the caller can apply it before any layout
/// event arrives.
fn setup_layout_tracking<F>(
    state: &Rc<RefCell<RuntimeState>>,
    config: &KeystrokeConfig,
    mode_active: &Arc<AtomicBool>,
    on_layout: F,
) -> Option<String>
where
    F: Fn(&str) + 'static,
{
    if !config.auto_detect_layout {
        return config.keyboard_layout.clone();
    }

    let mut layout_manager = LayoutManager::new();
    if let Err(e) = layout_manager.init() {
        warn!("Failed to initialize layout detection: {}", e);
    }

    let initial_layout = config
        .keyboard_layout
        .clone()
        .or_else(|| layout_manager.current_layout_name());

    let (layout_tx, layout_rx) = bounded::<LayoutEvent>(16);
    layout_manager.start_listener(move |event| {
        let _ = layout_tx.try_send(event);
    });

    state.borrow_mut().layout_manager = Some(layout_manager);

    let active = Arc::clone(mode_active);
    let state_for_task = Rc::clone(state);
    glib::MainContext::default().spawn_local(async move {
        while let Ok(event) = layout_rx.recv().await {
            if !active.load(Ordering::SeqCst) {
                break;
            }
            if state_for_task.borrow().routing_engine.get_states().0 == CaptureState::Paused {
                continue;
            }
            if let LayoutEvent::LayoutSwitched { name, .. } = event {
                on_layout(&name);
            }
        }
        debug!("Layout event loop terminated");
    });

    initial_layout
}

#[allow(clippy::too_many_lines)]
fn start_keystroke_mode(
    app: &Application,
    config_service: &ConfigService,
    state: &Rc<RefCell<RuntimeState>>,
) -> Result<()> {
    info!("Starting keystroke mode");

    let config = config_service.get_config();

    if let Some(engine) = state.borrow_mut().audio_engine.as_mut() {
        engine.start();
    }

    if let Some(dispatcher) = &state.borrow().audio_dispatcher {
        let current_pack = dispatcher.get_current_pack_name();
        let target_pack = &config.sound.sound_pack;

        info!(
            "Current pack: {:?}, Target pack: {}",
            current_pack, target_pack
        );

        if current_pack.is_none() || current_pack.as_deref() != Some(target_pack.as_str()) {
            info!("Loading initial sound pack: {}", target_pack);
            if let Err(e) = dispatcher.load_pack(target_pack, config.sound.folder.as_deref()) {
                warn!("Failed to load sound pack '{}': {}", target_pack, e);
            }
        }

        dispatcher.set_enabled(config.sound.enabled);
        dispatcher.set_volume(config.sound.volume);
    }

    {
        let mut s = state.borrow_mut();
        s.routing_engine.reset_mode_state();
    }

    let window = create_window(app, &config)?;

    if config.keystroke_draggable {
        let controllers = setup_drag(&window);
        state.borrow_mut().drag_controllers = controllers;
    }

    let display = Rc::new(RefCell::new(KeyDisplayWidget::new(&config)));

    let mode_active = Arc::new(AtomicBool::new(true));
    let visual_enabled = Rc::new(Cell::new(config.show_visual));

    let display_for_layout = Rc::clone(&display);
    let initial_layout = setup_layout_tracking(state, &config, &mode_active, move |name| {
        display_for_layout.borrow_mut().set_layout(name);
    });

    if let Some(ref name) = initial_layout {
        info!("Using keyboard layout: {}", name);
        display.borrow_mut().set_layout(name);
    }

    window.set_child(Some(display.borrow().widget()));

    let (sender, receiver) = bounded::<KeyEvent>(1024);

    let listener_config = ListenerConfig {
        all_keyboards: config.all_keyboards,
        ..Default::default()
    };

    let audio_dispatcher = state.borrow().audio_dispatcher.clone();
    let listener = KeyListener::new(sender, listener_config, audio_dispatcher);
    let handle = listener.start()?;

    {
        let mut s = state.borrow_mut();
        s.mode_active = Some(Arc::clone(&mode_active));
        s.visual_enabled = Some(Rc::clone(&visual_enabled));
        s.listener_handle = Some(handle);
        s.keystroke_window = Some(window.clone());
    }

    let active_key_loop = Arc::clone(&mode_active);
    let state_clone = Rc::clone(state);
    let display_clone = Rc::clone(&display);

    let app_clone = app.clone();
    let config_service_clone = config_service.clone();

    glib::MainContext::default().spawn_local(async move {
        while let Ok(event) = receiver.recv().await {
            if !active_key_loop.load(Ordering::SeqCst) {
                break;
            }

            let routing_result = match &event {
                KeyEvent::Pressed(kd) => state_clone.borrow_mut().routing_engine.process(
                    kd.key,
                    true,
                    false,
                    DisplayMode::Keystroke,
                ),
                KeyEvent::Released(kd) => state_clone.borrow_mut().routing_engine.process(
                    kd.key,
                    false,
                    false,
                    DisplayMode::Keystroke,
                ),
                KeyEvent::AllReleased => RoutingResult::Dispatch(evdev::Key::KEY_RESERVED, false),
            };

            match routing_result {
                RoutingResult::Ignored => {}
                RoutingResult::StateChanged(capture, focus) => {
                    if let Some(service) = &state_clone.borrow().feedback_service {
                        service.handle_state_change(capture, focus);
                    }
                }
                RoutingResult::SwitchMode(mode) => {
                    show_mode_feedback(&state_clone, mode);

                    let app = app_clone.clone();
                    let state = state_clone.clone();
                    let service = config_service_clone.clone();

                    glib::MainContext::default().spawn_local(async move {
                        switch_mode(&app, &state, &service, mode);
                    });
                    break;
                }
                RoutingResult::Dispatch(_, _) => {
                    let mut display = display_clone.borrow_mut();
                    match event {
                        KeyEvent::Pressed(key) => display.add_key(key),
                        KeyEvent::Released(key) => display.remove_key(&key),
                        KeyEvent::AllReleased => display.clear(),
                    }
                }
            }
        }
        debug!("Keystroke event loop terminated");
    });

    let mut rx = config_service.subscribe();
    let display_c = display.clone();
    let window_c = window.clone();
    let state_c = state.clone();
    let active_c = Arc::clone(&mode_active);
    let visual_c = Rc::clone(&visual_enabled);

    glib::MainContext::default().spawn_local(async move {
        while rx.changed().await.is_ok() {
            if !active_c.load(Ordering::SeqCst) {
                break;
            }
            let cfg = rx.borrow().clone();

            apply_hotkey_config(&state_c, &cfg);

            if let Some(dispatcher) = &state_c.borrow().audio_dispatcher {
                dispatcher.set_enabled(cfg.sound.enabled);
                dispatcher.set_volume(cfg.sound.volume);

                let current_pack = dispatcher.get_current_pack_name();
                let target_pack = &cfg.sound.sound_pack;

                if current_pack.as_deref() != Some(target_pack.as_str()) {
                    if let Err(e) = dispatcher.load_pack(target_pack, cfg.sound.folder.as_deref()) {
                        warn!("Failed to load sound pack '{}': {}", target_pack, e);
                    }
                }
            }

            {
                let mut disp = display_c.borrow_mut();
                disp.set_display_timeout(cfg.display_timeout_ms);
                disp.set_max_keys(cfg.max_keys);
                disp.set_separate_duplicate_keys(cfg.separate_duplicate_keys);
                disp.set_combine_modifier_keys(cfg.combine_modifier_keys);
                disp.set_max_combo_keys(cfg.max_combo_keys);
                disp.set_unify_words(cfg.unify_words);
                disp.set_max_word_length(cfg.max_word_length);
                disp.set_backspace_edits_word(cfg.backspace_edits_word);
                disp.set_super_key_icon(&cfg.super_key_icon);
            }

            if let Some(provider) = &state_c.borrow().css_provider {
                crate::ui::update_css_provider(provider, &cfg);
            }

            {
                let mut s = state_c.borrow_mut();
                let current_draggable = !s.drag_controllers.is_empty();

                if cfg.keystroke_draggable != current_draggable {
                    if cfg.keystroke_draggable {
                        let controllers = setup_drag(&window_c);
                        s.drag_controllers = controllers;
                    } else {
                        for c in &s.drag_controllers {
                            window_c.remove_controller(c);
                        }
                        s.drag_controllers.clear();

                        crate::ui::window::update_position(&window_c, cfg.position, cfg.margin);
                    }
                }
            }

            if !cfg.keystroke_draggable {
                crate::ui::window::update_position(&window_c, cfg.position, cfg.margin);
            }

            visual_c.set(cfg.show_visual);
            if !cfg.show_visual {
                window_c.set_visible(false);
            }
        }
    });

    let active = Arc::clone(&mode_active);
    let state_clone = Rc::clone(state);
    setup_keystroke_cleanup_timer(
        display.clone(),
        window.clone(),
        state_clone,
        active,
        Rc::clone(&visual_enabled),
    );

    if config.show_visual {
        window.present();
    }

    Ok(())
}

#[allow(clippy::too_many_lines)]
fn start_bubble_mode(
    app: &Application,
    config_service: &ConfigService,
    state: &Rc<RefCell<RuntimeState>>,
) -> Result<()> {
    info!("Starting bubble mode");

    let config = config_service.get_config();

    if let Some(engine) = state.borrow_mut().audio_engine.as_mut() {
        engine.start();
    }

    if let Some(dispatcher) = &state.borrow().audio_dispatcher {
        dispatcher.set_enabled(config.sound.enabled);
        dispatcher.set_volume(config.sound.volume);

        let current_pack = dispatcher.get_current_pack_name();
        let target_pack = &config.sound.sound_pack;

        if current_pack.as_deref() != Some(target_pack.as_str()) {
            if let Err(e) = dispatcher.load_pack(target_pack, config.sound.folder.as_deref()) {
                warn!("Failed to load sound pack '{}': {}", target_pack, e);
            }
        }
    }

    {
        let mut s = state.borrow_mut();
        s.routing_engine.reset_mode_state();
    }

    let window = create_bubble_window(app, &config)?;

    if config.bubble.draggable {
        let controllers = setup_drag(&window);
        state.borrow_mut().drag_controllers = controllers;
    }

    let display = Rc::new(RefCell::new(BubbleDisplayWidget::new(
        config.bubble.timeout_ms,
    )));
    display
        .borrow_mut()
        .set_alignment(alignment_for(config.bubble.position));

    let mode_active = Arc::new(AtomicBool::new(true));
    let visual_enabled = Rc::new(Cell::new(config.show_visual));

    let display_for_layout = Rc::clone(&display);
    let initial_layout = setup_layout_tracking(state, &config, &mode_active, move |name| {
        display_for_layout.borrow_mut().set_layout(name);
    });

    if let Some(ref name) = initial_layout {
        info!("Using keyboard layout: {}", name);
        display.borrow_mut().set_layout(name);
    }

    window.set_child(Some(display.borrow().widget()));

    let (sender, receiver) = bounded::<KeyEvent>(1024);

    let listener_config = ListenerConfig {
        all_keyboards: config.all_keyboards,
        ..Default::default()
    };

    let audio_dispatcher = state.borrow().audio_dispatcher.clone();
    let listener = KeyListener::new(sender, listener_config, audio_dispatcher);
    let handle = listener.start()?;

    {
        let s = state.borrow();
        let (capture, focus) = s.routing_engine.get_states();
        if capture == CaptureState::Active && focus == crate::domain::FocusState::Focused {
            handle.send_command(InputControlCommand::Grab);
        }
    }

    let mode_active = Arc::new(AtomicBool::new(true));
    {
        let mut s = state.borrow_mut();
        s.mode_active = Some(Arc::clone(&mode_active));
        s.visual_enabled = Some(Rc::clone(&visual_enabled));
        s.listener_handle = Some(handle);
        s.bubble_window = Some(window.clone());
    }

    let active = Arc::clone(&mode_active);
    let state_clone = Rc::clone(state);
    let display_clone = Rc::clone(&display);

    let app_clone = app.clone();
    let config_service_clone = config_service.clone();

    glib::MainContext::default().spawn_local(async move {
        let app = app_clone.clone();
        let config_service = config_service_clone.clone();

        while let Ok(event) = receiver.recv().await {
            if !active.load(Ordering::SeqCst) {
                break;
            }

            let routing_result = match &event {
                KeyEvent::Pressed(kd) => state_clone.borrow_mut().routing_engine.process(
                    kd.key,
                    true,
                    true,
                    DisplayMode::Bubble,
                ),
                KeyEvent::Released(kd) => state_clone.borrow_mut().routing_engine.process(
                    kd.key,
                    false,
                    true,
                    DisplayMode::Bubble,
                ),
                KeyEvent::AllReleased => RoutingResult::Dispatch(evdev::Key::KEY_RESERVED, false),
            };

            match routing_result {
                RoutingResult::Ignored => {}
                RoutingResult::StateChanged(capture, focus) => {
                    let settings_open = state_clone.borrow().settings_window.is_some();
                    if let Some(handle) = &state_clone.borrow().listener_handle {
                        if capture == CaptureState::Active
                            && focus == crate::domain::FocusState::Focused
                            && !settings_open
                        {
                            handle.send_command(InputControlCommand::Grab);
                        } else {
                            handle.send_command(InputControlCommand::Ungrab);
                        }
                    }

                    if let Some(service) = &state_clone.borrow().feedback_service {
                        service.handle_state_change(capture, focus);
                    }
                }
                RoutingResult::SwitchMode(mode) => {
                    show_mode_feedback(&state_clone, mode);

                    let app = app.clone();
                    let state = state_clone.clone();
                    let service = config_service.clone();

                    glib::MainContext::default().spawn_local(async move {
                        switch_mode(&app, &state, &service, mode);
                    });
                    break;
                }
                RoutingResult::Dispatch(_, _) => {
                    let mut display = display_clone.borrow_mut();
                    match event {
                        KeyEvent::Pressed(key) => {
                            display.process_key(key);
                        }
                        KeyEvent::Released(key) => {
                            display.process_key_release(key);
                        }
                        KeyEvent::AllReleased => {}
                    }
                }
            }
        }
        debug!("Bubble event loop terminated");
    });

    let _active = Arc::clone(&mode_active);
    let _state_clone = Rc::clone(state);
    let _display_clone = Rc::clone(&display);

    let mut rx = config_service.subscribe();
    let display_c = display.clone();
    let window_c = window.clone();
    let state_c = state.clone();
    let active_c = Arc::clone(&mode_active);
    let visual_c = Rc::clone(&visual_enabled);

    glib::MainContext::default().spawn_local(async move {
        while rx.changed().await.is_ok() {
            if !active_c.load(Ordering::SeqCst) {
                break;
            }
            let cfg = rx.borrow().clone();

            apply_hotkey_config(&state_c, &cfg);

            if let Some(dispatcher) = &state_c.borrow().audio_dispatcher {
                dispatcher.set_enabled(cfg.sound.enabled);
                dispatcher.set_volume(cfg.sound.volume);

                let current_pack = dispatcher.get_current_pack_name();
                let target_pack = &cfg.sound.sound_pack;

                if current_pack.as_deref() != Some(target_pack.as_str()) {
                    if let Err(e) = dispatcher.load_pack(target_pack, cfg.sound.folder.as_deref()) {
                        warn!("Failed to load sound pack '{}': {}", target_pack, e);
                    }
                }
            }

            {
                let mut disp = display_c.borrow_mut();
                disp.set_display_duration(cfg.bubble.timeout_ms);
                disp.set_alignment(alignment_for(cfg.bubble.position));
            }

            if let Some(provider) = &state_c.borrow().css_provider {
                crate::ui::update_css_provider(provider, &cfg);
            }

            {
                let mut s = state_c.borrow_mut();
                let current_draggable = !s.drag_controllers.is_empty();

                if cfg.bubble.draggable != current_draggable {
                    if cfg.bubble.draggable {
                        let controllers = setup_drag(&window_c);
                        s.drag_controllers = controllers;
                    } else {
                        for c in &s.drag_controllers {
                            window_c.remove_controller(c);
                        }
                        s.drag_controllers.clear();

                        crate::ui::window::update_position(
                            &window_c,
                            cfg.bubble.position,
                            cfg.margin,
                        );
                    }
                }
            }

            if !cfg.bubble.draggable {
                crate::ui::window::update_position(&window_c, cfg.bubble.position, cfg.margin);
            }

            visual_c.set(cfg.show_visual);
            if !cfg.show_visual {
                window_c.set_visible(false);
            }
        }
    });

    let active = Arc::clone(&mode_active);
    let state_clone = Rc::clone(state);
    setup_bubble_cleanup_timer(
        display.clone(),
        window.clone(),
        state_clone,
        active,
        Rc::clone(&visual_enabled),
    );

    if config.show_visual {
        window.present();
    }

    Ok(())
}

fn setup_keystroke_cleanup_timer(
    display: Rc<RefCell<KeyDisplayWidget>>,
    window: ApplicationWindow,
    state: Rc<RefCell<RuntimeState>>,
    mode_active: Arc<AtomicBool>,
    visual_enabled: Rc<Cell<bool>>,
) {
    glib::timeout_add_local(Duration::from_millis(100), move || {
        if !mode_active.load(Ordering::SeqCst) {
            return ControlFlow::Break;
        }

        if !visual_enabled.get() {
            window.set_visible(false);
            return ControlFlow::Continue;
        }

        if state.borrow().routing_engine.get_states().0 == CaptureState::Paused {
            return ControlFlow::Continue;
        }

        let mut display = display.borrow_mut();
        display.remove_expired();

        if display.has_keys() {
            window.remove_css_class("fading-out");
            window.set_visible(true);
        } else if !window.has_css_class("fading-out") {
            window.add_css_class("fading-out");

            let w = window.clone();
            glib::timeout_add_local_once(Duration::from_millis(200), move || {
                if w.has_css_class("fading-out") {
                    w.set_visible(false);
                }
            });
        }

        ControlFlow::Continue
    });
}

fn setup_bubble_cleanup_timer(
    display: Rc<RefCell<BubbleDisplayWidget>>,
    window: ApplicationWindow,
    state: Rc<RefCell<RuntimeState>>,
    mode_active: Arc<AtomicBool>,
    visual_enabled: Rc<Cell<bool>>,
) {
    glib::timeout_add_local(Duration::from_millis(100), move || {
        if !mode_active.load(Ordering::SeqCst) {
            return ControlFlow::Break;
        }

        if !visual_enabled.get() {
            window.set_visible(false);
            return ControlFlow::Continue;
        }

        if state.borrow().routing_engine.get_states().0 == CaptureState::Paused {
            return ControlFlow::Continue;
        }

        let mut display = display.borrow_mut();
        display.remove_expired();

        if display.should_show() {
            window.remove_css_class("fading-out");
            window.set_visible(true);
        } else if !window.has_css_class("fading-out") {
            window.add_css_class("fading-out");

            let w = window.clone();
            glib::timeout_add_local_once(Duration::from_millis(200), move || {
                if w.has_css_class("fading-out") {
                    w.set_visible(false);
                }
            });
        }

        ControlFlow::Continue
    });
}

fn load_css_defaults() -> CssProvider {
    let provider = gtk4::CssProvider::new();
    let defaults = include_str!("../style/defaults.css");
    let settings = include_str!("../style/settings.css");

    let bubble_css = include_str!("../style/bubble.css");

    provider.load_from_string(&format!("{defaults}\n{settings}\n{bubble_css}"));

    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    provider
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn actions(flags: &[&str]) -> Vec<AppAction> {
        let set: HashSet<&str> = flags.iter().copied().collect();
        actions_from_flags(|key| set.contains(key))
    }

    #[test]
    fn test_cli_no_flags_has_no_actions() {
        assert!(actions(&[]).is_empty());
    }

    #[test]
    fn test_cli_mode_switches() {
        assert_eq!(
            actions(&["keystroke-toggle"]),
            vec![AppAction::KeystrokeMode]
        );
        assert_eq!(actions(&["keystroke"]), vec![AppAction::KeystrokeMode]);
        assert_eq!(actions(&["bubble-toggle"]), vec![AppAction::BubbleMode]);
        assert_eq!(actions(&["bubble"]), vec![AppAction::BubbleMode]);
        assert_eq!(actions(&["toggle"]), vec![AppAction::ToggleMode]);
    }

    #[test]
    fn test_cli_control_flags() {
        assert_eq!(actions(&["pause-toggle"]), vec![AppAction::TogglePause]);
        assert_eq!(actions(&["settings"]), vec![AppAction::OpenSettings]);
        assert_eq!(actions(&["launcher"]), vec![AppAction::ShowLauncher]);
        assert_eq!(actions(&["quit"]), vec![AppAction::Quit]);
    }

    #[test]
    fn test_cli_sound_and_visual_flags() {
        assert_eq!(actions(&["sound-toggle"]), vec![AppAction::ToggleSound]);
        assert_eq!(actions(&["visual-toggle"]), vec![AppAction::ToggleVisual]);
    }

    #[test]
    fn test_cli_multiple_flags_preserve_order() {
        assert_eq!(
            actions(&["settings", "pause-toggle"]),
            vec![AppAction::TogglePause, AppAction::OpenSettings]
        );
    }
}
