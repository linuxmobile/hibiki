use anyhow::{Context, Result};
use evdev::Device;
use std::fs;
use std::path::PathBuf;
use tracing::{debug, info, warn};

#[derive(Debug, Clone)]
pub struct KeyboardDevice {
    pub path: PathBuf,

    pub name: String,
}

impl KeyboardDevice {
    pub fn open(&self) -> Result<Device> {
        Device::open(&self.path).with_context(|| format!("Failed to open device: {:?}", self.path))
    }
}

#[derive(Debug, Clone)]
pub struct MouseDevice {
    pub path: PathBuf,

    pub name: String,
}

impl MouseDevice {
    pub fn open(&self) -> Result<Device> {
        Device::open(&self.path).with_context(|| format!("Failed to open device: {:?}", self.path))
    }
}

pub fn discover_keyboards() -> Result<Vec<KeyboardDevice>> {
    let mut keyboards = Vec::new();
    let input_dir = PathBuf::from("/dev/input");

    let entries = fs::read_dir(&input_dir)
        .with_context(|| format!("Failed to read directory: {:?}", input_dir))?;

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        if !file_name.starts_with("event") {
            continue;
        }

        match Device::open(&path) {
            Ok(device) => {
                if is_keyboard(&device) {
                    let name = device.name().unwrap_or("Unknown Keyboard").to_string();

                    info!("Found keyboard: {} at {:?}", name, path);

                    keyboards.push(KeyboardDevice { path, name });
                }
            }
            Err(e) => {
                debug!("Could not open {:?}: {}", path, e);
            }
        }
    }

    if keyboards.is_empty() {
        warn!("No keyboard devices found. Ensure you are in the 'input' group.");
    }

    Ok(keyboards)
}

pub fn discover_mice() -> Result<Vec<MouseDevice>> {
    let mut mice = Vec::new();
    let input_dir = PathBuf::from("/dev/input");

    let entries = fs::read_dir(&input_dir)
        .with_context(|| format!("Failed to read directory: {:?}", input_dir))?;

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();

        if !file_name.starts_with("event") {
            continue;
        }

        match Device::open(&path) {
            Ok(device) => {
                if is_mouse(&device) {
                    let name = device.name().unwrap_or("Unknown Mouse").to_string();

                    info!("Found mouse: {} at {:?}", name, path);

                    mice.push(MouseDevice { path, name });
                }
            }
            Err(e) => {
                debug!("Could not open {:?}: {}", path, e);
            }
        }
    }

    if mice.is_empty() {
        warn!("No mouse devices found. Ensure you are in the 'input' group.");
    }

    Ok(mice)
}

fn is_keyboard(device: &Device) -> bool {
    let supported = device.supported_events();
    if !supported.contains(evdev::EventType::KEY) {
        return false;
    }

    if let Some(keys) = device.supported_keys() {
        let has_letter_keys = keys.contains(evdev::Key::KEY_A)
            && keys.contains(evdev::Key::KEY_Z)
            && keys.contains(evdev::Key::KEY_SPACE);

        return has_letter_keys;
    }

    false
}

fn is_mouse(device: &Device) -> bool {
    let supported = device.supported_events();

    if !supported.contains(evdev::EventType::KEY) {
        return false;
    }

    let has_mouse_buttons = device.supported_keys().map_or(false, |keys| {
        keys.contains(evdev::Key::BTN_LEFT)
            || keys.contains(evdev::Key::BTN_RIGHT)
            || keys.contains(evdev::Key::BTN_MIDDLE)
    });

    let has_rel_events = supported.contains(evdev::EventType::RELATIVE);

    has_mouse_buttons && has_rel_events
}

fn is_virtual(name: &str) -> bool {
    name.as_bytes()
        .windows(7)
        .any(|w| w.eq_ignore_ascii_case(b"virtual"))
}

#[allow(dead_code)]
pub fn get_primary_keyboard() -> Result<KeyboardDevice> {
    let keyboards = discover_keyboards()?;

    if let Some(kb) = keyboards.iter().find(|kb| !is_virtual(&kb.name)) {
        return Ok(kb.clone());
    }

    keyboards
        .into_iter()
        .next()
        .context("No keyboard devices found")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_returns_result() {
        let result = discover_keyboards();
        if let Err(e) = result {
            println!("Discovery failed as expected in sandbox: {}", e);
        }
    }
}
