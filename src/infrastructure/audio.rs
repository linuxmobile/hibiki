use crate::domain::audio::{KeyDefine, KeyDefineType, MechvibesConfig};
use anyhow::{Context, Result};
use include_dir::{include_dir, Dir};
use rodio::{Decoder, Source};
use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::result::Result::Ok;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct AudioBuffer {
    pub samples: Arc<[i16]>,
    pub sample_rate: u32,
    pub channels: u16,
}

impl AudioBuffer {
    pub fn to_source(&self) -> AudioBufferSource {
        AudioBufferSource {
            buffer: self.samples.clone(),
            pos: 0,
            sample_rate: self.sample_rate,
            channels: self.channels,
            end_pos: self.samples.len(),
        }
    }

    pub fn to_source_slice(&self, start_ms: u64, duration_ms: u64) -> AudioBufferSource {
        let start_pos =
            (start_ms * self.sample_rate as u64 / 1000) as usize * self.channels as usize;
        let duration_pos =
            (duration_ms * self.sample_rate as u64 / 1000) as usize * self.channels as usize;
        let end_pos = (start_pos + duration_pos).min(self.samples.len());

        AudioBufferSource {
            buffer: self.samples.clone(),
            pos: start_pos,
            sample_rate: self.sample_rate,
            channels: self.channels,
            end_pos,
        }
    }
}

pub struct AudioBufferSource {
    buffer: Arc<[i16]>,
    pos: usize,
    sample_rate: u32,
    channels: u16,
    end_pos: usize,
}

impl Iterator for AudioBufferSource {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos < self.end_pos {
            let res = self.buffer[self.pos] as f32 / 32767.0;
            self.pos += 1;
            Some(res)
        } else {
            None
        }
    }
}

impl Source for AudioBufferSource {
    fn current_span_len(&self) -> Option<usize> {
        None
    }

    fn channels(&self) -> u16 {
        self.channels
    }

    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f64(
            (self.end_pos - self.pos) as f64 / self.sample_rate as f64 / self.channels as f64,
        ))
    }
}

#[derive(Debug)]
pub struct LoadedSoundPack {
    pub config: MechvibesConfig,
    pub buffers: HashMap<String, AudioBuffer>,
}

pub struct SoundPackLoader;

static BUILTIN_SOUNDS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/src/assets/builtin-sounds");

const SYSTEM_SOUND_BASES: &[&str] = &["/usr/share/hibiki/sounds", "/usr/local/share/hibiki/sounds"];

trait PackReader {
    fn read_bytes(&self, rel: &str) -> Result<Vec<u8>>;
}

struct FsPackReader {
    root: PathBuf,
}

impl PackReader for FsPackReader {
    fn read_bytes(&self, rel: &str) -> Result<Vec<u8>> {
        let path = SoundPackLoader::safe_join(&self.root, rel)?;
        fs::read(&path).with_context(|| format!("Failed to read sound file: {:?}", path))
    }
}

struct EmbeddedPackReader {
    dir: &'static Dir<'static>,
}

impl PackReader for EmbeddedPackReader {
    fn read_bytes(&self, rel: &str) -> Result<Vec<u8>> {
        self.dir
            .files()
            .find(|file| file.path().file_name().and_then(|name| name.to_str()) == Some(rel))
            .map(|file| file.contents().to_vec())
            .with_context(|| format!("Failed to read embedded sound file: {}", rel))
    }
}

impl SoundPackLoader {
    fn safe_join(base: &Path, sub: &str) -> Result<PathBuf> {
        let sub_path = Path::new(sub);
        if sub_path.is_absolute()
            || sub_path
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            anyhow::bail!("Forbidden path component: {}", sub);
        }

        let joined = base.join(sub_path);

        if !joined.exists() {
            return Ok(joined);
        }

        let canon_joined = joined
            .canonicalize()
            .with_context(|| format!("Failed to canonicalize path: {:?}", joined))?;
        let canon_base = base
            .canonicalize()
            .with_context(|| format!("Failed to canonicalize base path: {:?}", base))?;

        if !canon_joined.starts_with(canon_base) {
            anyhow::bail!("Path traversal detected via symlink: {}", sub);
        }

        Ok(canon_joined)
    }

    pub fn user_sounds_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("hibiki")
            .join("sounds")
    }

    pub fn ensure_user_sounds_dir() -> PathBuf {
        let dir = Self::user_sounds_dir();
        let _ = fs::create_dir_all(&dir);
        dir
    }

    fn system_dirs() -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        if let Ok(env_path) = std::env::var("HIBIKI_SOUNDS_DIR") {
            let path = PathBuf::from(env_path);
            if path.is_dir() {
                dirs.push(path);
            }
        }

        for base in SYSTEM_SOUND_BASES {
            let path = PathBuf::from(base);
            if path.is_dir() {
                dirs.push(path);
            }
        }

        dirs
    }

    fn display_name_from_config(bytes: &[u8], fallback: &str) -> String {
        serde_json::from_slice::<serde_json::Value>(bytes)
            .ok()
            .and_then(|json| {
                json.get("name")
                    .and_then(|value| value.as_str())
                    .map(str::to_string)
            })
            .unwrap_or_else(|| fallback.to_string())
    }

    pub fn list_available_packs(extra_dir: Option<&str>) -> Vec<(String, String)> {
        let mut packs = std::collections::BTreeMap::<String, String>::new();

        for dir in BUILTIN_SOUNDS.dirs() {
            let Some(id) = dir.path().file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            let display = dir
                .files()
                .find(|file| {
                    file.path().file_name().and_then(|name| name.to_str()) == Some("config.json")
                })
                .map_or_else(
                    || id.to_string(),
                    |file| Self::display_name_from_config(file.contents(), id),
                );
            packs.insert(id.to_string(), display);
        }

        let mut dirs = Vec::new();
        if let Some(extra) = extra_dir {
            dirs.push(PathBuf::from(extra));
        }
        dirs.push(Self::user_sounds_dir());
        dirs.extend(Self::system_dirs());

        for dir in dirs {
            let Ok(entries) = fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                if !entry.file_type().map(|kind| kind.is_dir()).unwrap_or(false) {
                    continue;
                }
                let Ok(id) = entry.file_name().into_string() else {
                    continue;
                };
                let display = fs::read(entry.path().join("config.json")).map_or_else(
                    |_| id.clone(),
                    |bytes| Self::display_name_from_config(&bytes, &id),
                );
                packs.insert(id, display);
            }
        }

        packs.into_iter().collect()
    }

    fn reader_for(id: &str, extra_dir: Option<&str>) -> Option<Box<dyn PackReader>> {
        if let Some(extra) = extra_dir {
            let path = Path::new(extra).join(id);
            if path.is_dir() {
                return Some(Box::new(FsPackReader { root: path }));
            }
        }

        let user = Self::user_sounds_dir().join(id);
        if user.is_dir() {
            return Some(Box::new(FsPackReader { root: user }));
        }

        for base in Self::system_dirs() {
            let path = base.join(id);
            if path.is_dir() {
                return Some(Box::new(FsPackReader { root: path }));
            }
        }

        BUILTIN_SOUNDS
            .get_dir(id)
            .map(|dir| Box::new(EmbeddedPackReader { dir }) as Box<dyn PackReader>)
    }

    pub fn load_pack(id: &str, extra_dir: Option<&str>) -> Result<LoadedSoundPack> {
        let reader = Self::reader_for(id, extra_dir)
            .with_context(|| format!("Sound pack '{id}' not found"))?;
        Self::load_from_reader(reader.as_ref())
    }

    fn decode_buffer(bytes: &[u8]) -> Result<AudioBuffer> {
        let decoder =
            Decoder::new(Cursor::new(bytes.to_vec())).context("Failed to decode sound")?;
        let sample_rate = decoder.sample_rate();
        let channels = decoder.channels();
        let samples: Vec<i16> = decoder.map(|sample| (sample * 32767.0) as i16).collect();
        Ok(AudioBuffer {
            samples: Arc::from(samples),
            sample_rate,
            channels,
        })
    }

    fn load_from_reader(reader: &dyn PackReader) -> Result<LoadedSoundPack> {
        let config_bytes = reader.read_bytes("config.json")?;
        let mut config: MechvibesConfig =
            serde_json::from_slice(&config_bytes).context("Failed to parse config.json")?;

        let mut buffers = HashMap::new();

        match config.key_define_type {
            KeyDefineType::Single => {
                let bytes = reader.read_bytes(&config.sound)?;
                buffers.insert("main".to_string(), Self::decode_buffer(&bytes)?);
            }
            KeyDefineType::Multi => {
                for define in config.defines.values() {
                    if let KeyDefine::Multi(Some(rel_path)) = define {
                        if let Ok(bytes) = reader.read_bytes(rel_path) {
                            if let Ok(buffer) = Self::decode_buffer(&bytes) {
                                buffers.insert(rel_path.clone(), buffer);
                            }
                        }
                    }
                }
            }
        }

        config.path = PathBuf::new();
        Ok(LoadedSoundPack { config, buffers })
    }

    pub fn load_from_directory(path: &Path) -> Result<LoadedSoundPack> {
        if path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        {
            anyhow::bail!("Path traversal detected: {:?}", path);
        }

        let root = path
            .canonicalize()
            .with_context(|| format!("Failed to canonicalize pack path: {:?}", path))?;

        let reader = FsPackReader { root };
        Self::load_from_reader(&reader)
    }
}
