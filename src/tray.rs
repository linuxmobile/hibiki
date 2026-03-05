use async_channel::Sender;
use ksni::{self, menu::StandardItem, Icon, MenuItem, Tray, TrayService};
use parking_lot::Mutex;
use std::sync::Arc;
use tracing::{debug, error, info};

#[derive(Debug, Clone)]
pub enum TrayAction {
    ShowLauncher,
    KeystrokeMode,
    BubbleMode,
    OpenSettings,
    TogglePause,
    Quit,
}

pub struct TrayState {
    pub paused: bool,
    pub icon_pixmap: Vec<Icon>,
}

impl Default for TrayState {
    fn default() -> Self {
        Self {
            paused: false,
            icon_pixmap: generate_icon_pixmap(),
        }
    }
}

struct KeystrokeTray {
    action_sender: Sender<TrayAction>,
    state: Arc<Mutex<TrayState>>,
}

fn generate_icon_pixmap() -> Vec<Icon> {
    let size = 22;
    let mut data = Vec::with_capacity(size * size * 4);

    for y in 0..size {
        for x in 0..size {
            let (a, r, g, b) = if is_keyboard_pixel(x, y, size) {
                (220, 128, 128, 128)
            } else {
                (0, 0, 0, 0)
            };
            data.push(a);
            data.push(r);
            data.push(g);
            data.push(b);
        }
    }

    vec![Icon {
        width: size as i32,
        height: size as i32,
        data,
    }]
}

fn is_keyboard_pixel(x: usize, y: usize, size: usize) -> bool {
    let margin = 2;

    let body_left = margin;
    let body_right = size - margin - 1;
    let body_top = size / 3;
    let body_bottom = size - margin - 1;

    let on_body_outline = (y >= body_top && y <= body_bottom)
        && ((x == body_left || x == body_right)
            || (y == body_top || y == body_bottom)
            || (x == body_left + 1 || x == body_right - 1)
            || (y == body_top + 1 && x >= body_left && x <= body_right));

    let key_row_1 = body_top + 3;
    let key_row_2 = body_top + 6;
    let key_row_3 = body_top + 9;

    let on_key =
        (y >= body_top + 2 && y <= body_bottom - 2 && x >= body_left + 2 && x <= body_right - 2)
            && ((y == key_row_1 || y == key_row_2 || y == key_row_3) && !x.is_multiple_of(3));

    on_body_outline || on_key
}

impl Tray for KeystrokeTray {
    fn id(&self) -> String {
        "hibiki".to_string()
    }

    fn title(&self) -> String {
        "Hibiki".to_string()
    }

    fn icon_name(&self) -> String {
        String::new()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        self.state.lock().icon_pixmap.clone()
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let status = if self.state.lock().paused {
            "Paused"
        } else {
            "Running"
        };
        ksni::ToolTip {
            icon_name: String::new(),
            icon_pixmap: Vec::new(),
            title: "Hibiki".to_string(),
            description: format!("Hibiki Visualizer - {}", status),
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        debug!("Tray left-clicked: showing launcher");
        if let Err(e) = self.action_sender.send_blocking(TrayAction::ShowLauncher) {
            error!("Failed to send tray action: {}", e);
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let pause_label = if self.state.lock().paused {
            "Resume"
        } else {
            "Pause"
        };

        vec![
            MenuItem::Standard(StandardItem {
                label: "Keystroke Mode".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    debug!("Tray: Keystroke mode selected");
                    let _ = tray.action_sender.send_blocking(TrayAction::KeystrokeMode);
                }),
                ..Default::default()
            }),
            MenuItem::Standard(StandardItem {
                label: "Bubble Mode".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    debug!("Tray: Bubble mode selected");
                    let _ = tray.action_sender.send_blocking(TrayAction::BubbleMode);
                }),
                ..Default::default()
            }),
            MenuItem::Separator,
            MenuItem::Standard(StandardItem {
                label: "Settings".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    debug!("Tray: Settings selected");
                    let _ = tray.action_sender.send_blocking(TrayAction::OpenSettings);
                }),
                ..Default::default()
            }),
            MenuItem::Standard(StandardItem {
                label: pause_label.to_string(),
                activate: Box::new(|tray: &mut Self| {
                    debug!("Tray: Toggle pause selected");
                    let _ = tray.action_sender.send_blocking(TrayAction::TogglePause);
                }),
                ..Default::default()
            }),
            MenuItem::Separator,
            MenuItem::Standard(StandardItem {
                label: "Quit".to_string(),
                activate: Box::new(|tray: &mut Self| {
                    debug!("Tray: Quit selected");
                    let _ = tray.action_sender.send_blocking(TrayAction::Quit);
                }),
                ..Default::default()
            }),
        ]
    }
}

#[derive(Clone)]
pub struct TrayHandle {
    service_handle: ksni::Handle<KeystrokeTray>,
    state: Arc<Mutex<TrayState>>,
}

impl TrayHandle {
    pub fn set_paused(&self, paused: bool) {
        self.state.lock().paused = paused;

        self.service_handle.update(|_| {});
    }

    pub fn set_icon(&self, icon: Vec<Icon>) {
        self.state.lock().icon_pixmap = icon;

        self.service_handle.update(|_| {});
    }

    #[allow(dead_code)]
    pub fn is_paused(&self) -> bool {
        self.state.lock().paused
    }
}

pub fn start_tray() -> anyhow::Result<(async_channel::Receiver<TrayAction>, TrayHandle)> {
    let (sender, receiver) = async_channel::bounded(32);
    let state = Arc::new(Mutex::new(TrayState::default()));

    let tray = KeystrokeTray {
        action_sender: sender,
        state: Arc::clone(&state),
    };

    let service = TrayService::new(tray);
    let handle = TrayHandle {
        service_handle: service.handle(),
        state,
    };

    service.spawn();

    info!("System tray started");

    Ok((receiver, handle))
}
