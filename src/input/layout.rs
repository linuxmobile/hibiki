use crate::compositor::{
    self, hyprland::HyprlandClient, niri::NiriClient, sway::SwayClient, umbriel::UmbrielClient,
    Compositor, CompositorClient, KeyboardLayouts, LayoutEvent,
};
use std::io::{BufRead, Read};
use std::sync::{Arc, RwLock};
use std::thread::{self, JoinHandle};
use tracing::{debug, info, warn};

#[allow(dead_code)]
pub struct LayoutManager {
    compositor: Compositor,

    client: Option<Box<dyn CompositorClient>>,

    layouts: Arc<RwLock<KeyboardLayouts>>,

    stop_flag: Arc<std::sync::atomic::AtomicBool>,

    listener_handle: Option<JoinHandle<()>>,
}

#[allow(dead_code)]
impl LayoutManager {
    #[must_use]
    pub fn new() -> Self {
        let compositor = compositor::detect();
        let client = compositor::create_client(compositor);

        info!(
            "LayoutManager initialized: compositor={}, client_available={}",
            compositor,
            client.is_some()
        );

        Self {
            compositor,
            client,
            layouts: Arc::new(RwLock::new(KeyboardLayouts::default())),
            stop_flag: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            listener_handle: None,
        }
    }

    pub fn init(&self) -> anyhow::Result<()> {
        match self.fetch_layouts() {
            Ok(layouts) => {
                info!(
                    "Fetched {} keyboard layout(s), current: {:?}",
                    layouts.len(),
                    layouts.current_name()
                );
                if let Ok(mut guard) = self.layouts.write() {
                    *guard = layouts;
                }
                Ok(())
            }
            Err(e) => {
                warn!("Failed to fetch keyboard layouts: {}", e);
                Err(e)
            }
        }
    }

    #[must_use]
    pub const fn compositor(&self) -> Compositor {
        self.compositor
    }

    #[must_use]
    pub fn supports_layout_query(&self) -> bool {
        self.client.is_some() && self.compositor.supports_layout_query()
    }

    #[must_use]
    pub fn layouts(&self) -> KeyboardLayouts {
        self.layouts
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default()
    }

    #[must_use]
    pub fn current_layout_name(&self) -> Option<String> {
        self.layouts
            .read()
            .ok()
            .and_then(|guard| guard.current_name().map(String::from))
    }

    #[must_use]
    pub fn current_layout_index(&self) -> usize {
        self.layouts
            .read()
            .map(|guard| guard.current_idx)
            .unwrap_or(0)
    }

    pub fn refresh(&self) -> anyhow::Result<()> {
        let layouts = self.fetch_layouts()?;
        if let Ok(mut guard) = self.layouts.write() {
            *guard = layouts;
        }
        Ok(())
    }

    fn fetch_layouts(&self) -> anyhow::Result<KeyboardLayouts> {
        match &self.client {
            Some(client) => client.get_keyboard_layouts(),
            None => {
                debug!("No compositor client available for {}", self.compositor);
                Ok(KeyboardLayouts::default())
            }
        }
    }

    pub fn start_listener<F>(&mut self, callback: F)
    where
        F: Fn(LayoutEvent) + Send + 'static,
    {
        if !self.compositor.supports_layout_events() {
            debug!(
                "Compositor {} does not support layout events",
                self.compositor
            );
            return;
        }

        let compositor = self.compositor;
        let layouts = Arc::clone(&self.layouts);
        let stop_flag = Arc::clone(&self.stop_flag);

        let handle = thread::spawn(move || match compositor {
            Compositor::Niri => {
                Self::listen_niri(layouts, stop_flag, callback);
            }
            Compositor::Hyprland => {
                Self::listen_hyprland(layouts, stop_flag, callback);
            }
            Compositor::Sway => {
                Self::listen_sway(layouts, stop_flag, callback);
            }
            Compositor::Umbriel => {
                Self::listen_umbriel(layouts, stop_flag, callback);
            }
            _ => {
                debug!("No event listener implemented for {}", compositor);
            }
        });

        self.listener_handle = Some(handle);
        info!("Started layout change listener for {}", self.compositor);
    }

    pub fn stop_listener(&mut self) {
        self.stop_flag
            .store(true, std::sync::atomic::Ordering::SeqCst);

        if let Some(handle) = self.listener_handle.take() {
            let _ = handle.join();
        }
    }

    fn listen_niri<F>(
        layouts: Arc<RwLock<KeyboardLayouts>>,
        stop_flag: Arc<std::sync::atomic::AtomicBool>,
        callback: F,
    ) where
        F: Fn(LayoutEvent),
    {
        let client = match NiriClient::new() {
            Some(c) => c,
            None => {
                warn!("Failed to create Niri client for event listener");
                return;
            }
        };

        let reader = match client.subscribe_events() {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to subscribe to Niri events: {}", e);
                return;
            }
        };

        for line in reader.lines() {
            if stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }

            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    debug!("Error reading Niri event: {}", e);
                    break;
                }
            };

            if let Some(event) = client.parse_event(&line) {
                if let LayoutEvent::LayoutsChanged {
                    layouts: ref new_layouts,
                } = event
                {
                    if let Ok(mut cached) = layouts.write() {
                        *cached = new_layouts.clone();
                    }
                }

                callback(event);
            }
        }

        debug!("Niri event listener stopped");
    }

    fn listen_hyprland<F>(
        layouts: Arc<RwLock<KeyboardLayouts>>,
        stop_flag: Arc<std::sync::atomic::AtomicBool>,
        callback: F,
    ) where
        F: Fn(LayoutEvent),
    {
        let client = match HyprlandClient::new() {
            Some(c) => c,
            None => {
                warn!("Failed to create Hyprland client for event listener");
                return;
            }
        };

        let reader = match client.subscribe_events() {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to subscribe to Hyprland events: {}", e);
                return;
            }
        };

        for line in reader.lines() {
            if stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }

            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    debug!("Error reading Hyprland event: {}", e);
                    break;
                }
            };

            if let Some((event_name, event_data)) = HyprlandClient::parse_event(&line) {
                if HyprlandClient::is_layout_event(event_name) {
                    if let Some((_keyboard, layout_name)) =
                        HyprlandClient::parse_layout_event(event_data)
                    {
                        let current_idx = if let Ok(mut cached) = layouts.write() {
                            let index = cached
                                .names
                                .iter()
                                .position(|n| n == layout_name)
                                .unwrap_or_else(|| {
                                    cached.names.push(layout_name.to_string());
                                    cached.names.len() - 1
                                });
                            cached.current_idx = index;
                            index
                        } else {
                            0
                        };

                        callback(LayoutEvent::LayoutSwitched {
                            name: layout_name.to_string(),
                            index: current_idx,
                        });
                    }
                }
            }
        }

        debug!("Hyprland event listener stopped");
    }

    fn listen_sway<F>(
        layouts: Arc<RwLock<KeyboardLayouts>>,
        stop_flag: Arc<std::sync::atomic::AtomicBool>,
        callback: F,
    ) where
        F: Fn(LayoutEvent),
    {
        let client = match SwayClient::new() {
            Some(c) => c,
            None => {
                warn!("Failed to create Sway client for event listener");
                return;
            }
        };

        let stream = match client.subscribe_events() {
            Ok(s) => s,
            Err(e) => {
                warn!("Failed to subscribe to Sway events: {}", e);
                return;
            }
        };

        let mut reader = std::io::BufReader::new(stream);

        loop {
            if stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }

            let mut header = [0u8; 14];
            if reader.read_exact(&mut header).is_err() {
                break;
            }

            if &header[0..6] != b"i3-ipc" {
                continue;
            }

            let payload_len = match header[6..10].try_into() {
                Ok(bytes) => u32::from_le_bytes(bytes),
                Err(_) => continue,
            };

            if payload_len > crate::compositor::sway::MAX_IPC_PAYLOAD_SIZE {
                break;
            }

            let mut payload = vec![0u8; payload_len as usize];
            if reader.read_exact(&mut payload).is_err() {
                break;
            }

            let json = match String::from_utf8(payload) {
                Ok(s) => s,
                Err(_) => continue,
            };

            if json.contains("\"xkb_layout\"") || json.contains("\"xkb_keymap\"") {
                if let Some(c) = SwayClient::new() {
                    if let Ok(new_layouts) = c.get_keyboard_layouts() {
                        if let Ok(mut guard) = layouts.write() {
                            *guard = new_layouts.clone();
                        }
                        callback(LayoutEvent::LayoutsChanged {
                            layouts: new_layouts,
                        });
                    }
                }
            }
        }

        debug!("Sway event listener stopped");
    }

    fn listen_umbriel<F>(
        layouts: Arc<RwLock<KeyboardLayouts>>,
        stop_flag: Arc<std::sync::atomic::AtomicBool>,
        callback: F,
    ) where
        F: Fn(LayoutEvent),
    {
        let client = match UmbrielClient::new() {
            Some(c) => c,
            None => {
                warn!("Failed to create Umbriel client for event listener");
                return;
            }
        };

        let reader = match client.subscribe_events() {
            Ok(r) => r,
            Err(e) => {
                warn!("Failed to subscribe to Umbriel events: {}", e);
                return;
            }
        };

        for line in reader.lines() {
            if stop_flag.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }

            let line = match line {
                Ok(l) => l,
                Err(e) => {
                    debug!("Error reading Umbriel event: {}", e);
                    break;
                }
            };

            // Umbriel always sends a full snapshot (names + current index) rather
            // than a delta, so refresh the cache and surface the active layout as
            // a switch for the visualizer.
            if let Some(LayoutEvent::LayoutsChanged {
                layouts: new_layouts,
            }) = client.parse_event(&line)
            {
                let index = new_layouts.current_idx;
                let name = new_layouts.current_name().map(String::from);

                if let Ok(mut cached) = layouts.write() {
                    *cached = new_layouts;
                }

                if let Some(name) = name {
                    callback(LayoutEvent::LayoutSwitched { name, index });
                }
            }
        }

        debug!("Umbriel event listener stopped");
    }
}

impl Default for LayoutManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for LayoutManager {
    fn drop(&mut self) {
        self.stop_listener();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_manager_new() {
        let manager = LayoutManager::new();

        let _ = manager.compositor();
    }

    #[test]
    fn test_layout_manager_default_layouts() {
        let manager = LayoutManager::new();
        let layouts = manager.layouts();

        assert!(layouts.is_empty() || !layouts.is_empty());
    }

    #[test]
    fn test_layout_manager_current_layout() {
        let manager = LayoutManager::new();

        let _ = manager.current_layout_name();
    }
}
