use crate::domain::audio::KeyDefine;
use crate::infrastructure::audio::{LoadedSoundPack, SoundPackLoader};
use crate::input::keymap::map_evdev_to_mechvibes;
use anyhow::Result;
use parking_lot::RwLock;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{OutputStream, OutputStreamBuilder, Source};
use std::path::PathBuf;
use std::sync::mpsc::SyncSender;
use std::sync::Arc;

pub struct AudioEngine {
    #[allow(dead_code)]
    state: Arc<RwLock<AudioState>>,
    cmd_rx: Option<std::sync::mpsc::Receiver<PlayCommand>>,
}

struct AudioState {
    pack: Option<LoadedSoundPack>,
    volume: f32,
    enabled: bool,
    pack_name: Option<String>,
}

enum PlayCommand {
    Play(Box<dyn Source<Item = f32> + Send>),
}

#[derive(Clone)]
pub struct AudioDispatcher {
    state: Arc<RwLock<AudioState>>,
    cmd_tx: SyncSender<PlayCommand>,
}

fn try_open_stream() -> Result<OutputStream, String> {
    let hosts = rodio::cpal::available_hosts();
    for host_id in &hosts {
        let host_name = format!("{:?}", host_id);
        if host_name.contains("Jack") {
            tracing::info!("Found JACK host, attempting to use it");
            if let Ok(host) = rodio::cpal::host_from_id(*host_id) {
                if let Some(device) = host.default_output_device() {
                    if let Ok(builder) = OutputStreamBuilder::from_device(device) {
                        if let Ok(stream) = builder.open_stream() {
                            tracing::info!("Successfully opened stream via JACK backend");
                            return Ok(stream);
                        }
                    }
                }
            }
        }
    }

    match OutputStreamBuilder::open_default_stream() {
        Ok(s) => return Ok(s),
        Err(e) => tracing::warn!("Failed to open default audio stream: {}", e),
    }

    for host_id in hosts {
        tracing::debug!("Trying audio host: {:?}", host_id);
        let host = match rodio::cpal::host_from_id(host_id) {
            Ok(h) => h,
            Err(_) => continue,
        };

        if let Ok(devices) = host.output_devices() {
            for device in devices {
                let device_name = device.name().unwrap_or_else(|_| "unknown".to_string());
                tracing::info!("  Trying device: {}", device_name);

                match OutputStreamBuilder::from_device(device) {
                    Ok(builder) => {
                        if let Ok(stream) = builder.open_stream() {
                            tracing::info!(
                                "  Successfully opened stream on device: {}",
                                device_name
                            );
                            return Ok(stream);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to create builder for device {}: {}", device_name, e)
                    }
                }
            }
        }
    }

    Err("No working audio output device found on any host".to_string())
}

impl AudioEngine {
    pub fn new() -> Result<(Self, AudioDispatcher)> {
        let state = Arc::new(RwLock::new(AudioState {
            pack: None,
            volume: 1.0,
            enabled: false,
            pack_name: None,
        }));

        let (cmd_tx, cmd_rx) = std::sync::mpsc::sync_channel::<PlayCommand>(1024);

        let dispatcher = AudioDispatcher {
            state: state.clone(),
            cmd_tx,
        };

        Ok((
            Self {
                state,
                cmd_rx: Some(cmd_rx),
            },
            dispatcher,
        ))
    }

    pub fn start(&mut self) {
        if let Some(cmd_rx) = self.cmd_rx.take() {
            std::thread::spawn(move || {
                tracing::info!("Audio thread started (Lazy Init)");

                match try_open_stream() {
                    Ok(stream) => {
                        tracing::info!("Audio output stream opened successfully");
                        let mixer = stream.mixer();

                        while let Ok(cmd) = cmd_rx.recv() {
                            match cmd {
                                PlayCommand::Play(source_box) => {
                                    mixer.add(source_box);
                                }
                            }
                        }
                        tracing::info!("Audio thread shutting down");
                    }
                    Err(e) => {
                        tracing::error!("Audio thread failed to open output stream: {}", e);

                        let default_host = rodio::cpal::default_host();
                        tracing::info!("Default audio host: {:?}", default_host.id());

                        if let Ok(devices) = default_host.output_devices() {
                            tracing::info!("Available output devices on default host:");
                            for (i, device) in devices.enumerate() {
                                let name = device.name().unwrap_or_else(|_| "unknown".to_string());
                                tracing::info!("  Device {}: {}", i, name);
                            }
                        } else {
                            tracing::error!("Failed to list output devices");
                        }

                        tracing::info!("All available hosts:");
                        for host_id in rodio::cpal::available_hosts() {
                            tracing::info!("  {:?}", host_id);
                        }
                    }
                }
            });
        }
    }
}

impl AudioDispatcher {
    pub fn load_pack(&self, path: PathBuf) -> Result<()> {
        let pack_name = path.file_name().map(|n| n.to_string_lossy().to_string());

        let pack = SoundPackLoader::load_from_directory(&path)?;
        let mut state = self.state.write();
        state.pack = Some(pack);
        state.pack_name = pack_name;
        Ok(())
    }

    pub fn get_current_pack_name(&self) -> Option<String> {
        let state = self.state.read();
        state.pack_name.clone()
    }

    pub fn set_volume(&self, volume: f32) {
        let mut state = self.state.write();
        state.volume = volume.clamp(0.0, 1.0);
    }

    pub fn set_enabled(&self, enabled: bool) {
        let mut state = self.state.write();
        state.enabled = enabled;
    }

    pub fn play_key(&self, keycode: u16) {
        let state = self.state.read();
        if !state.enabled {
            return;
        }

        if let Some(pack) = &state.pack {
            let raw_key = keycode.to_string();

            let mapped_key = map_evdev_to_mechvibes(keycode).map(|k| k.to_string());

            let fallback_key = "30".to_string();

            let target_key = if pack.config.defines.contains_key(&raw_key) {
                Some(raw_key)
            } else if let Some(mapped) = mapped_key
                .as_ref()
                .filter(|k| pack.config.defines.contains_key(*k))
            {
                Some(mapped.clone())
            } else if pack.config.defines.contains_key(&fallback_key) {
                Some(fallback_key)
            } else {
                None
            };

            if let Some(key_to_play) = target_key {
                if let Some(define) = pack.config.defines.get(&key_to_play) {
                    match define {
                        KeyDefine::Multi(Some(filename)) => {
                            if let Some(buffer) = pack.buffers.get(filename) {
                                let source = buffer.to_source().amplify(state.volume);
                                let _ = self.cmd_tx.try_send(PlayCommand::Play(Box::new(source)));
                            }
                        }
                        KeyDefine::Single(range) => {
                            if range.len() >= 2 {
                                let start = range[0];
                                let duration = range[1];
                                if let Some(buffer) = pack.buffers.get("main") {
                                    let source = buffer
                                        .to_source_slice(start, duration)
                                        .amplify(state.volume);
                                    let _ =
                                        self.cmd_tx.try_send(PlayCommand::Play(Box::new(source)));
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}
