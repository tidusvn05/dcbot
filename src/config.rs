use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// User-editable settings at ~/.config/dcbot/config.toml
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    /// UI language: en | vi | ja. None = auto-detect / English.
    pub lang: Option<String>,
}

pub const VALID_LANGS: &[&str] = &["en", "vi", "ja"];

pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("dcbot")
}

pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn bots_file() -> PathBuf {
    config_dir().join("bots.toml")
}

pub fn load() -> Config {
    let Ok(raw) = fs::read_to_string(config_file()) else {
        return Config::default();
    };
    toml::from_str(&raw).unwrap_or_default()
}

pub fn set_lang(lang: &str) -> Result<()> {
    let cfg = Config {
        lang: Some(lang.to_string()),
    };
    let dir = config_dir();
    fs::create_dir_all(&dir)?;
    fs::write(config_file(), toml::to_string_pretty(&cfg)?)?;
    Ok(())
}

/// Resolve effective locale: flag > DCBOT_LANG > config > system LANG > en.
/// Pass `flag = None` for the pre-parse pass (before --lang is known).
pub fn resolve_lang(flag: Option<&str>) -> String {
    if let Some(l) = flag {
        return normalize(l);
    }
    if let Ok(l) = std::env::var("DCBOT_LANG") {
        if !l.is_empty() {
            return normalize(&l);
        }
    }
    if let Some(l) = load().lang {
        return normalize(&l);
    }
    for var in ["LC_ALL", "LANG"] {
        if let Ok(l) = std::env::var(var) {
            if let Some(code) = l.get(..2) {
                if VALID_LANGS.contains(&code) {
                    return code.to_string();
                }
            }
        }
    }
    "en".to_string()
}

fn normalize(l: &str) -> String {
    let l = l.trim().to_lowercase();
    if VALID_LANGS.contains(&l.as_str()) {
        l
    } else {
        "en".to_string()
    }
}
