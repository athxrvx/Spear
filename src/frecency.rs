use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct FrecencyStore {
    // Map of item_id -> list of Unix timestamps (in seconds)
    pub accesses: HashMap<String, Vec<u64>>,
}

impl FrecencyStore {
    pub fn get_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/atharva".to_string());
        PathBuf::from(home).join(".config").join("spear").join("frecency.json")
    }

    pub fn load() -> Self {
        let path = Self::get_path();
        if !path.exists() {
            return Self::default();
        }
        if let Ok(content) = fs::read_to_string(path) {
            if let Ok(store) = serde_json::from_str::<Self>(&content) {
                return store;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        #[cfg(test)]
        {
            Ok(())
        }
        #[cfg(not(test))]
        {
            let path = Self::get_path();
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let content = serde_json::to_string_pretty(self)?;
            fs::write(path, content)
        }
    }

    pub fn record_access(&mut self, item_id: &str) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        
        let entry = self.accesses.entry(item_id.to_string()).or_default();
        entry.push(now);
        
        // Keep at most 100 entries per item to keep files compact
        if entry.len() > 100 {
            entry.drain(0..entry.len() - 100);
        }
        
        let _ = self.save();
    }

    pub fn get_frecency_score(&self, item_id: &str, now: u64) -> i32 {
        let mut score = 0;
        if let Some(accesses) = self.accesses.get(item_id) {
            for &t in accesses {
                if t > now {
                    continue;
                }
                let dt = now - t;
                let weight = if dt < 3600 {
                    100
                } else if dt < 86400 {
                    80
                } else if dt < 7 * 86400 {
                    60
                } else if dt < 30 * 86400 {
                    30
                } else {
                    10
                };
                score += weight;
            }
        }
        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frecency_decay() {
        let mut store = FrecencyStore::default();
        let now = 1000000;
        
        // 1. No accesses -> 0 score
        assert_eq!(store.get_frecency_score("item1", now), 0);

        // 2. Access 30 minutes ago (1800s ago)
        store.accesses.insert("item1".to_string(), vec![now - 1800]);
        assert_eq!(store.get_frecency_score("item1", now), 100);

        // 3. Multiple accesses at different times:
        // - 30 minutes ago (100)
        // - 12 hours ago (80)
        // - 5 days ago (60)
        // Total = 240
        store.accesses.insert(
            "item1".to_string(),
            vec![now - 1800, now - 12 * 3600, now - 5 * 24 * 3600],
        );
        assert_eq!(store.get_frecency_score("item1", now), 240);
    }

    #[test]
    fn test_pruning() {
        let mut store = FrecencyStore::default();
        // Record 120 accesses
        for _ in 0..120 {
            store.record_access("item1");
        }
        // Should keep exactly 100 accesses
        assert_eq!(store.accesses.get("item1").unwrap().len(), 100);
    }
}
