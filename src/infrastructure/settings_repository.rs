use crate::domain::config::{KeystrokeConfig, SoundConfig, Validate};
use anyhow::{Context, Result};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;
use tracing::{debug, info, warn};

pub struct SettingsRepository {
    config_dir: PathBuf,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
struct LegacySoundLayout {
    sound: Option<SoundConfig>,
    audio: Option<SoundConfig>,
    bubble: LegacyBubbleSound,
}

#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
struct LegacyBubbleSound {
    audio: Option<SoundConfig>,
}

impl LegacySoundLayout {
    fn migrate_into(&self, config: &mut KeystrokeConfig) {
        if self.sound.is_some() {
            return;
        }
        if let Some(legacy) = self.audio.as_ref().or(self.bubble.audio.as_ref()) {
            config.sound = legacy.clone();
        }
    }
}

impl SettingsRepository {
    pub fn new() -> Result<Self> {
        let config_dir = dirs::config_dir()
            .context("Could not determine config directory")?
            .join("hibiki");
        Ok(Self { config_dir })
    }

    pub fn with_dir(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }

    pub fn load(&self) -> Result<KeystrokeConfig> {
        let toml_path = self.config_dir.join("config.toml");
        let json_path = self.config_dir.join("config.json");

        if toml_path.exists() {
            let content = fs::read_to_string(&toml_path)
                .with_context(|| format!("Failed to read config: {:?}", toml_path))?;
            let mut config: KeystrokeConfig =
                toml::from_str(&content).with_context(|| "Failed to parse TOML config file")?;
            if let Ok(legacy) = toml::from_str::<LegacySoundLayout>(&content) {
                legacy.migrate_into(&mut config);
            }
            config.validate();
            info!("Loaded configuration from {:?}", toml_path);
            Ok(config)
        } else if json_path.exists() {
            info!(
                "Found legacy JSON config at {:?}, migrating to TOML...",
                json_path
            );
            let content = fs::read_to_string(&json_path)
                .with_context(|| format!("Failed to read legacy config: {:?}", json_path))?;
            let mut config: KeystrokeConfig = serde_json::from_str(&content)
                .with_context(|| "Failed to parse legacy JSON config")?;
            if let Ok(legacy) = serde_json::from_str::<LegacySoundLayout>(&content) {
                legacy.migrate_into(&mut config);
            }
            config.validate();

            self.save(&config)?;

            if let Err(e) = fs::remove_file(&json_path) {
                warn!("Failed to remove legacy config file {:?}: {}", json_path, e);
            } else {
                info!("Successfully migrated and removed legacy config file");
            }

            Ok(config)
        } else {
            debug!("No config file found, using defaults");
            Ok(KeystrokeConfig::default())
        }
    }

    pub fn save(&self, config: &KeystrokeConfig) -> Result<()> {
        if !self.config_dir.exists() {
            fs::create_dir_all(&self.config_dir)
                .with_context(|| format!("Failed to create config dir: {:?}", self.config_dir))?;
        }

        let content =
            toml::to_string_pretty(config).context("Failed to serialize config to TOML")?;
        let toml_path = self.config_dir.join("config.toml");

        let mut temp_file = NamedTempFile::new_in(&self.config_dir)
            .with_context(|| format!("Failed to create temp file in {:?}", self.config_dir))?;

        temp_file
            .write_all(content.as_bytes())
            .context("Failed to write to temp config file")?;

        temp_file
            .persist(&toml_path)
            .with_context(|| format!("Failed to persist config to {:?}", toml_path))?;

        info!("Saved configuration to {:?}", toml_path);
        Ok(())
    }
}
