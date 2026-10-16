use crate::application::audio_dispatcher::AudioDispatcher;
use crate::input::device::{discover_keyboards, KeyboardDevice};
use crate::input::keymap::KeyDisplay;
use anyhow::{Context, Result};
use async_channel::{Receiver, Sender, TrySendError};
use evdev::{Device, InputEventKind, Key};
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use std::collections::HashSet;
use std::os::fd::{AsRawFd, BorrowedFd};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tracing::{error, info, trace, warn};

#[derive(Debug, Clone)]
pub enum KeyEvent {
    Pressed(KeyDisplay),
    Released(KeyDisplay),
    #[allow(dead_code)]
    AllReleased,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputControlCommand {
    Grab,
    Ungrab,
}

#[derive(Debug, Clone)]
pub struct ListenerConfig {
    pub all_keyboards: bool,
    pub ignored_keys: HashSet<Key>,
}

impl Default for ListenerConfig {
    fn default() -> Self {
        Self {
            all_keyboards: true,
            ignored_keys: HashSet::new(),
        }
    }
}

pub struct ListenerHandle {
    running: Arc<AtomicBool>,
    control_txs: Vec<Sender<InputControlCommand>>,
    threads: Vec<thread::JoinHandle<()>>,
}

impl ListenerHandle {
    pub fn send_command(&self, cmd: InputControlCommand) {
        for tx in &self.control_txs {
            if let Err(e) = tx.try_send(cmd) {
                warn!("Failed to send input control command: {}", e);
            }
        }
    }
}

impl Drop for ListenerHandle {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

pub struct KeyListener {
    sender: Sender<KeyEvent>,
    running: Arc<AtomicBool>,
    config: ListenerConfig,
    audio_dispatcher: Option<AudioDispatcher>,
}

impl KeyListener {
    #[must_use]
    pub fn new(
        sender: Sender<KeyEvent>,
        config: ListenerConfig,
        audio_dispatcher: Option<AudioDispatcher>,
    ) -> Self {
        Self {
            sender,
            running: Arc::new(AtomicBool::new(false)),
            config,
            audio_dispatcher,
        }
    }

    pub fn start(&self) -> Result<ListenerHandle> {
        let keyboards = discover_keyboards()?;

        if keyboards.is_empty() {
            anyhow::bail!("No keyboard devices found. Ensure you are in the 'input' group.");
        }

        let devices_to_use: Vec<KeyboardDevice> = if self.config.all_keyboards {
            keyboards
        } else {
            keyboards.into_iter().take(1).collect()
        };

        self.running.store(true, Ordering::SeqCst);

        let mut control_txs = Vec::new();
        let mut threads = Vec::new();

        for keyboard in devices_to_use {
            let sender = self.sender.clone();
            let running = Arc::clone(&self.running);
            let ignored_keys = self.config.ignored_keys.clone();
            let audio_dispatcher = self.audio_dispatcher.clone();

            let (control_tx, control_rx) = async_channel::unbounded::<InputControlCommand>();
            control_txs.push(control_tx);

            let handle = thread::spawn(move || {
                if let Err(e) = listen_to_device(
                    keyboard,
                    sender,
                    running,
                    ignored_keys,
                    control_rx,
                    audio_dispatcher,
                ) {
                    error!("Keyboard listener error: {}", e);
                }
            });
            threads.push(handle);
        }

        Ok(ListenerHandle {
            running: self.running.clone(),
            control_txs,
            threads,
        })
    }
}

fn listen_to_device(
    keyboard: KeyboardDevice,
    sender: Sender<KeyEvent>,
    running: Arc<AtomicBool>,
    ignored_keys: HashSet<Key>,
    control_rx: Receiver<InputControlCommand>,
    audio_dispatcher: Option<AudioDispatcher>,
) -> Result<()> {
    let mut device = keyboard.open()?;
    info!("Listening to keyboard: {}", keyboard.name);

    let raw_fd = device.as_raw_fd();
    let mut pressed_keys = HashSet::new();

    if let Ok(key_state) = device.get_key_state() {
        for key in key_state.iter() {
            if crate::input::keymap::is_modifier(key) {
                info!("Detected held modifier on startup: {:?}", key);
                pressed_keys.insert(key);
                let _ = sender.try_send(KeyEvent::Pressed(KeyDisplay::new(key, false)));
            }
        }
    }

    let borrowed_fd = unsafe { BorrowedFd::borrow_raw(raw_fd) };
    let mut poll_fds = [PollFd::new(borrowed_fd, PollFlags::POLLIN)];
    let mut is_grabbed = false;

    let mut pending_grab = false;

    while running.load(Ordering::SeqCst) {
        while let Ok(cmd) = control_rx.try_recv() {
            match cmd {
                InputControlCommand::Grab => {
                    if !is_grabbed {
                        if pressed_keys.is_empty() {
                            match device.grab() {
                                Ok(_) => {
                                    info!("Grabbed keyboard: {}", keyboard.name);
                                    is_grabbed = true;
                                    pending_grab = false;
                                }
                                Err(e) => {
                                    if e.raw_os_error() == Some(nix::libc::EBUSY) {
                                        warn!(
                                            "Keyboard {} busy; will retry the grab",
                                            keyboard.name
                                        );
                                        pending_grab = true;
                                    } else {
                                        error!("Failed to grab keyboard {}: {}", keyboard.name, e);
                                    }
                                }
                            }
                        } else {
                            info!(
                                "Deferring grab for {} until {} keys are released",
                                keyboard.name,
                                pressed_keys.len()
                            );
                            pending_grab = true;
                        }
                    }
                }
                InputControlCommand::Ungrab => {
                    pending_grab = false;
                    if is_grabbed {
                        match device.ungrab() {
                            Ok(_) => {
                                info!("Ungrabbed keyboard: {}", keyboard.name);
                                is_grabbed = false;
                            }
                            Err(e) => error!("Failed to ungrab keyboard {}: {}", keyboard.name, e),
                        }
                    }
                }
            }
        }

        let poll_result = poll(&mut poll_fds, PollTimeout::from(100_u16));

        match poll_result {
            Ok(_n) => {
                if let Err(e) = process_events(
                    &mut device,
                    &sender,
                    &ignored_keys,
                    &mut pressed_keys,
                    &audio_dispatcher,
                ) {
                    if e.to_string().contains("Channel closed") {
                        info!("Channel closed, stopping listener for {}", keyboard.name);
                        break;
                    }
                    warn!("Error processing events: {}", e);
                }

                if pending_grab && !is_grabbed && pressed_keys.is_empty() {
                    match device.grab() {
                        Ok(_) => {
                            info!("Executing pending grab for: {}", keyboard.name);
                            is_grabbed = true;
                            pending_grab = false;
                        }
                        Err(e) => {
                            if e.raw_os_error() == Some(nix::libc::EBUSY) {
                                warn!("Keyboard {} still busy; will retry the grab", keyboard.name);
                            } else {
                                error!(
                                    "Failed to execute pending grab for {}: {}",
                                    keyboard.name, e
                                );
                                pending_grab = false;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                error!("Poll error: {}", e);
                break;
            }
        }
    }

    if is_grabbed {
        let _ = device.ungrab();
    }

    info!("Stopped listening to keyboard: {}", keyboard.name);
    Ok(())
}

fn process_events(
    device: &mut Device,
    sender: &Sender<KeyEvent>,
    ignored_keys: &HashSet<Key>,
    pressed_keys: &mut HashSet<Key>,
    audio_dispatcher: &Option<AudioDispatcher>,
) -> Result<()> {
    let events = device.fetch_events().context("Failed to fetch events")?;
    let mut activity = false;

    for event in events {
        if let InputEventKind::Key(key) = event.kind() {
            if ignored_keys.contains(&key) {
                continue;
            }

            activity = true;
            let key_event = match event.value() {
                1 => {
                    trace!("Key pressed: {:?}", key);
                    pressed_keys.insert(key);
                    if let Some(dispatcher) = audio_dispatcher {
                        dispatcher.play_key(key.code());
                    }
                    KeyEvent::Pressed(KeyDisplay::new(key, true))
                }
                0 => {
                    trace!("Key released: {:?}", key);
                    pressed_keys.remove(&key);
                    KeyEvent::Released(KeyDisplay::new(key, false))
                }
                2 => {
                    trace!("Key repeat: {:?}", key);
                    KeyEvent::Pressed(KeyDisplay::new_repeat(key))
                }
                _ => continue,
            };

            if let Err(e) = sender.try_send(key_event) {
                match e {
                    TrySendError::Full(_) => warn!("Channel full, dropping event"),
                    TrySendError::Closed(_) => {
                        return Err(anyhow::anyhow!("Channel closed"));
                    }
                }
            }
        }
    }

    if activity && !pressed_keys.is_empty() {
        if let Ok(actual_state) = device.get_key_state() {
            let stuck_keys: Vec<Key> = pressed_keys
                .iter()
                .filter(|k| !actual_state.contains(**k))
                .cloned()
                .collect();

            for key in stuck_keys {
                trace!("Detected stuck key released (process): {:?}", key);
                pressed_keys.remove(&key);
                let key_display = KeyDisplay::new(key, false);
                let _ = sender.try_send(KeyEvent::Released(key_display));
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_channel::bounded;

    #[test]
    fn test_listener_config_default() {
        let config = ListenerConfig::default();
        assert!(config.all_keyboards);
        assert!(config.ignored_keys.is_empty());
    }

    #[test]
    fn test_listener_handle_drop() {
        let running = Arc::new(AtomicBool::new(true));
        let (control_tx, _) = bounded(1);
        let handle = ListenerHandle {
            running: running.clone(),
            control_txs: vec![control_tx],
            threads: Vec::new(),
        };

        assert!(running.load(Ordering::SeqCst));
        drop(handle);
        assert!(!running.load(Ordering::SeqCst));
    }
}
