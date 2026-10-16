use crate::domain::config::KeystrokeConfig;
use crate::infrastructure::settings_repository::SettingsRepository;
use anyhow::Result;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tracing::error;

#[derive(Clone)]
pub struct ConfigService {
    tx: Arc<watch::Sender<KeystrokeConfig>>,
    rx: watch::Receiver<KeystrokeConfig>,
}

impl ConfigService {
    pub fn new(repository: SettingsRepository) -> Result<Self> {
        let config = repository.load()?;
        let (tx, rx) = watch::channel(config);
        let repository = Arc::new(repository);

        let mut rx_clone = rx.clone();
        let repository_clone = Arc::clone(&repository);

        std::thread::spawn(move || {
            let rt = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(e) => {
                    error!("Failed to build background config runtime: {}", e);
                    return;
                }
            };

            rt.block_on(async move {
                let mut last_saved_config = rx_clone.borrow().clone();

                loop {
                    if rx_clone.changed().await.is_err() {
                        break;
                    }

                    tokio::time::sleep(Duration::from_millis(500)).await;

                    let new_config = rx_clone.borrow_and_update().clone();
                    if new_config != last_saved_config {
                        if let Err(e) = repository_clone.save(&new_config) {
                            error!("Failed to save configuration in background: {}", e);
                        } else {
                            last_saved_config = new_config;
                        }
                    }
                }
            });
        });

        Ok(Self {
            tx: Arc::new(tx),
            rx,
        })
    }

    #[must_use]
    pub fn get_config(&self) -> KeystrokeConfig {
        self.rx.borrow().clone()
    }

    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<KeystrokeConfig> {
        self.rx.clone()
    }

    pub fn update_config(&self, config: KeystrokeConfig) -> Result<()> {
        self.tx.send_replace(config);
        Ok(())
    }
}
