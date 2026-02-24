use keystroke::application::config_service::ConfigService;
use keystroke::domain::config::{KeystrokeConfig, Position};
use keystroke::infrastructure::settings_repository::SettingsRepository;
use std::fs;

#[tokio::test]
async fn test_config_service_updates() {
    let temp_dir = tempfile::tempdir().unwrap();
    let repo = SettingsRepository::with_dir(temp_dir.path().to_path_buf());
    let service = ConfigService::new(repo).unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let mut rx = service.subscribe();
    let mut cfg = service.get_config();
    cfg.max_keys = 20;
    cfg.position = Position::TopLeft;
    service.update_config(cfg.clone()).unwrap();

    rx.changed().await.unwrap();
    let current = rx.borrow().clone();
    assert_eq!(current.max_keys, 20);
    assert_eq!(current.position, Position::TopLeft);

    let mut cfg = service.get_config();
    cfg.bubble.timeout_ms = 5000;
    cfg.bubble.position = Position::BottomRight;
    service.update_config(cfg.clone()).unwrap();

    rx.changed().await.unwrap();
    let current = rx.borrow().clone();
    assert_eq!(current.bubble.timeout_ms, 5000);
    assert_eq!(current.bubble.position, Position::BottomRight);

    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

    let config_path = temp_dir.path().join("config.toml");

    let mut attempts = 0;
    let content = loop {
        match fs::read_to_string(&config_path) {
            Ok(c) => break c,
            Err(e) if attempts < 5 => {
                attempts += 1;
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                continue;
            }
            Err(e) => panic!("Failed to read config after retries: {}", e),
        }
    };
    let persisted: KeystrokeConfig = toml::from_str(&content).unwrap();
    assert_eq!(persisted.bubble.timeout_ms, 5000);
}
