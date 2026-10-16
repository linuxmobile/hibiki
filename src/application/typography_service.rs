use crate::infrastructure::font_provider::{FontError, FontProvider};
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct TypographyService;

impl TypographyService {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    pub async fn get_system_fonts(&self) -> Result<Arc<Vec<String>>, FontError> {
        let fonts = FontProvider::load_system_fonts_asynchronous().await?;

        let filtered: Vec<String> = fonts
            .iter()
            .filter(|f| {
                if f.starts_with("Noto Sans ") || f.starts_with("Noto Serif ") {
                    let allowed = [
                        "Noto Sans",
                        "Noto Serif",
                        "Noto Sans Mono",
                        "Noto Serif Mono",
                        "Noto Sans Display",
                        "Noto Serif Display",
                    ];
                    return allowed.contains(&f.as_str());
                }
                true
            })
            .cloned()
            .collect();

        Ok(Arc::new(filtered))
    }
}
