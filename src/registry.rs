use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::config;

/// Index of known deployments at ~/.config/dcbot/bots.toml.
/// Source of truth for bot details is each dir's bot.toml — this is a pointer map.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Registry {
    #[serde(default)]
    pub bots: BTreeMap<String, Entry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub dir: PathBuf,
    pub bot_user_id: String,
    pub bot_tag: String,
    pub created_at: String,
}

impl Registry {
    pub fn load() -> Registry {
        let Ok(raw) = fs::read_to_string(config::bots_file()) else {
            return Registry::default();
        };
        toml::from_str(&raw).unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        let file = config::bots_file();
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp = file.with_extension("toml.tmp");
        fs::write(&tmp, toml::to_string_pretty(self)?)?;
        fs::rename(&tmp, &file)?;
        Ok(())
    }

    pub fn add(&mut self, name: &str, entry: Entry) {
        self.bots.insert(name.to_string(), entry);
    }

    pub fn remove(&mut self, name: &str) -> Option<Entry> {
        self.bots.remove(name)
    }

    /// Find the registered name for a deployment dir (canonicalized compare).
    pub fn name_for_dir(&self, dir: &std::path::Path) -> Option<String> {
        let want = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        self.bots.iter().find_map(|(name, e)| {
            let have = std::fs::canonicalize(&e.dir).unwrap_or_else(|_| e.dir.clone());
            if have == want {
                Some(name.clone())
            } else {
                None
            }
        })
    }
}
