use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// bot.toml inside each deployment dir — the local source of truth.
#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub bot_user_id: String,
    pub bot_tag: String,
    pub created_at: String,
    #[serde(default = "default_channels_flag")]
    pub channels_flag: String,
    #[serde(default)]
    pub autorestart: bool,
}

pub fn default_channels_flag() -> String {
    "plugin:discord@claude-plugins-official".to_string()
}

pub fn file_path(dir: &Path) -> PathBuf {
    dir.join("bot.toml")
}

pub fn load(dir: &Path) -> Option<Manifest> {
    let raw = fs::read_to_string(file_path(dir)).ok()?;
    toml::from_str(&raw).ok()
}

pub fn save(dir: &Path, m: &Manifest) -> Result<()> {
    fs::write(file_path(dir), toml::to_string_pretty(m)?)?;
    Ok(())
}
