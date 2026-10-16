use hibiki::domain::config::{KeystrokeConfig, Position};
use hibiki::infrastructure::settings_repository::SettingsRepository;
use std::fs;

#[test]
fn test_migration_from_json_to_toml() {
    let temp_dir = tempfile::tempdir().unwrap();
    let json_path = temp_dir.path().join("config.json");
    let toml_path = temp_dir.path().join("config.toml");

    let legacy_config = r#"{
        "max_keys": 42,
        "position": "topleft",
        "display_mode": "keystroke",
        "bubble": {
            "timeout_ms": 1234
        }
    }"#;
    fs::write(&json_path, legacy_config).unwrap();

    let repo = SettingsRepository::with_dir(temp_dir.path().to_path_buf());

    let config = repo.load().expect("Failed to load config");

    assert_eq!(config.max_keys, 42);
    assert_eq!(config.position, Position::TopLeft);
    assert_eq!(config.bubble.timeout_ms, 1234);

    assert!(toml_path.exists(), "config.toml should exist");
    assert!(!json_path.exists(), "config.json should be deleted");

    let toml_content = fs::read_to_string(&toml_path).unwrap();
    let loaded_toml: KeystrokeConfig = toml::from_str(&toml_content).unwrap();
    assert_eq!(loaded_toml.max_keys, 42);
}

#[test]
fn test_toml_priority() {
    let temp_dir = tempfile::tempdir().unwrap();
    let json_path = temp_dir.path().join("config.json");
    let toml_path = temp_dir.path().join("config.toml");

    let legacy_config = r#"{ "max_keys": 10 }"#;
    fs::write(&json_path, legacy_config).unwrap();

    let new_config = r#"max_keys = 20"#;
    fs::write(&toml_path, new_config).unwrap();

    let repo = SettingsRepository::with_dir(temp_dir.path().to_path_buf());

    let config = repo.load().expect("Failed to load config");

    assert_eq!(config.max_keys, 20);

    assert!(
        json_path.exists(),
        "config.json should still exist if config.toml was already present"
    );
}

#[test]
fn test_legacy_keystroke_audio_migrates_to_sound() {
    let temp_dir = tempfile::tempdir().unwrap();
    let toml_path = temp_dir.path().join("config.toml");

    let legacy = r#"
[audio]
enabled = true
volume = 0.5
sound_pack = "legacy-pack"
"#;
    fs::write(&toml_path, legacy).unwrap();

    let repo = SettingsRepository::with_dir(temp_dir.path().to_path_buf());
    let config = repo.load().expect("Failed to load config");

    assert!(config.sound.enabled);
    assert_eq!(config.sound.volume, 0.5);
    assert_eq!(config.sound.sound_pack, "legacy-pack");
}

#[test]
fn test_legacy_bubble_audio_migrates_to_sound() {
    let temp_dir = tempfile::tempdir().unwrap();
    let toml_path = temp_dir.path().join("config.toml");

    let legacy = r#"
[bubble.audio]
enabled = true
volume = 0.25
sound_pack = "bubble-pack"
"#;
    fs::write(&toml_path, legacy).unwrap();

    let repo = SettingsRepository::with_dir(temp_dir.path().to_path_buf());
    let config = repo.load().expect("Failed to load config");

    assert!(config.sound.enabled);
    assert_eq!(config.sound.volume, 0.25);
    assert_eq!(config.sound.sound_pack, "bubble-pack");
}

#[test]
fn test_new_sound_section_wins_over_legacy_audio() {
    let temp_dir = tempfile::tempdir().unwrap();
    let toml_path = temp_dir.path().join("config.toml");

    let content = r#"
[audio]
enabled = true
sound_pack = "old-pack"

[sound]
enabled = true
sound_pack = "new-pack"
"#;
    fs::write(&toml_path, content).unwrap();

    let repo = SettingsRepository::with_dir(temp_dir.path().to_path_buf());
    let config = repo.load().expect("Failed to load config");

    assert_eq!(config.sound.sound_pack, "new-pack");
}
