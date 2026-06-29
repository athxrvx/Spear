use gio::prelude::*;
use std::cell::RefCell;
use crate::plugins::{Action, ResultItem, Plugin};

pub struct AppsPlugin {
    apps: RefCell<Vec<gio::AppInfo>>,
}

impl AppsPlugin {
    pub fn new() -> Self {
        let plugin = Self {
            apps: RefCell::new(Vec::new()),
        };
        plugin.reload_cache();
        plugin
    }
}

impl Plugin for AppsPlugin {
    fn id(&self) -> &str {
        "apps"
    }

    fn name(&self) -> &str {
        "Applications"
    }

    fn reload_cache(&self) {
        let all_apps = gio::AppInfo::all();
        *self.apps.borrow_mut() = all_apps;
    }

    fn query(&self, query_text: &str, _settings: &crate::settings::AppSettings) -> Vec<ResultItem> {
        let apps = self.apps.borrow();
        if query_text.trim().is_empty() {
            // Load frecency store to surface most frecent apps
            let frecency = crate::frecency::FrecencyStore::load();
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            let mut apps_with_score: Vec<(gio::AppInfo, i32)> = apps
                .iter()
                .filter(|app| app.should_show())
                .map(|app| {
                    let name = app.name().to_string();
                    let id = app
                        .id()
                        .map(|id| id.to_string())
                        .unwrap_or_else(|| name.clone());
                    let score = frecency.get_frecency_score(&id, now);
                    (app.clone(), score)
                })
                .collect();

            // Sort by frecency score desc, then by name alphabetically
            apps_with_score.sort_by(|a, b| {
                b.1.cmp(&a.1).then_with(|| a.0.name().to_lowercase().cmp(&b.0.name().to_lowercase()))
            });

            return apps_with_score
                .into_iter()
                .take(10)
                .map(|(app, score)| self.format_app(&app, score))
                .collect();
        }

        let query_lower = query_text.to_lowercase();
        let mut results = Vec::new();

        for app in apps.iter() {
            if !app.should_show() {
                continue;
            }

            let name = app.name().to_string();
            let desc = app.description().map(|d| d.to_string()).unwrap_or_default();
            let exec = app.executable().to_string_lossy().to_string();

            let mut score = 0;
            let name_lower = name.to_lowercase();

            if name_lower == query_lower {
                score = 100;
            } else if name_lower.starts_with(&query_lower) {
                score = 80;
            } else if name_lower.contains(&query_lower) {
                score = 60;
            } else if desc.to_lowercase().contains(&query_lower) {
                score = 40;
            } else if exec.to_lowercase().contains(&query_lower) {
                score = 20;
            }

            if score > 0 {
                results.push((self.format_app(app, score), score));
            }
        }

        // Sort by score descending, then by name alphabetically
        results.sort_by(|a, b| {
            b.1.cmp(&a.1).then_with(|| a.0.title.to_lowercase().cmp(&b.0.title.to_lowercase()))
        });

        results.into_iter().map(|(item, _)| item).collect()
    }
}

impl AppsPlugin {
    fn format_app(&self, app: &gio::AppInfo, score: i32) -> ResultItem {
        let name = app.name().to_string();
        let desc = app.description().map(|d| d.to_string());
        let exec = app.executable().to_string_lossy().to_string();
        let exec_opt = if exec.is_empty() { None } else { Some(exec) };
        
        let id = app
            .id()
            .map(|id| id.to_string())
            .unwrap_or_else(|| name.clone());

        let icon_str = app
            .icon()
            .and_then(|icon| icon.to_string().map(|s| s.to_string()))
            .unwrap_or_else(|| "application-x-executable".to_string());

        ResultItem {
            id: format!("app-{}", id),
            title: name.clone(),
            subtitle: desc.or(exec_opt),
            icon: Some(icon_str),
            category: "Applications".to_string(),
            score,
            actions: vec![Action {
                label: format!("Launch {}", name),
                action_type: "launch-app".to_string(),
                value: id, // Store the application ID to look it up on launch
            }],
        }
    }
}
