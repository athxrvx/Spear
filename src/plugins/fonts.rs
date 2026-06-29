use crate::plugins::{Action, ResultItem, Plugin};
use font_kit::source::SystemSource;

pub struct FontSearchPlugin;

impl FontSearchPlugin {
    pub fn new() -> Self {
        Self
    }
}

impl Plugin for FontSearchPlugin {
    fn id(&self) -> &str {
        "fonts"
    }

    fn name(&self) -> &str {
        "Font Search"
    }

    fn mode_keyword(&self) -> Option<&str> {
        Some("font")
    }

    fn is_mode_only(&self) -> bool {
        true
    }

    fn query(&self, query_text: &str, _settings: &crate::settings::AppSettings) -> Vec<ResultItem> {
        if query_text.is_empty() {
            return vec![];
        }

        let source = SystemSource::new();
        let families = match source.all_families() {
            Ok(f) => f,
            Err(_) => return vec![],
        };

        let query_lower = query_text.to_lowercase();
        let mut results = Vec::new();

        for family in families {
            if family.to_lowercase().contains(&query_lower) {
                results.push(ResultItem {
                    id: format!("font-{}", family),
                    title: family.clone(),
                    subtitle: Some("System Font".to_string()),
                    icon: Some("font-x-generic-symbolic".to_string()),
                    category: "Fonts".to_string(),
                    score: 90,
                    actions: vec![
                        Action {
                            label: "Copy Font Name".to_string(),
                            action_type: "copy-to-clipboard".to_string(),
                            value: family.clone(),
                        },
                        Action {
                            label: "Preview Font".to_string(),
                            action_type: "run-command".to_string(),
                            value: format!("gnome-font-viewer --family '{}'", family),
                        }
                    ],
                });
            }
            if results.len() >= 20 { break; }
        }

        results
    }
}
