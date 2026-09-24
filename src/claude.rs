use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The discord channel plugin and its marketplace.
pub const PLUGIN_ID: &str = "discord@claude-plugins-official";
const MARKETPLACE: &str = "claude-plugins-official";
const MARKETPLACE_REPO: &str = "anthropics/claude-plugins-official";

#[derive(Debug, PartialEq, Eq)]
pub enum PluginState {
    /// Installed, enabled, and usable for sessions running in `dir`.
    Ready,
    /// Install record usable for `dir` exists but enabledPlugins=false.
    Disabled,
    /// No install record applies to `dir`.
    Missing,
}

fn claude_home() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude"))
}

fn read_json(path: PathBuf) -> Option<Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

/// The discord@* install record usable for `dir`: user-scoped records
/// apply everywhere; project/local records only in their projectPath.
fn usable_install(dir: &Path) -> Option<String> {
    let doc = read_json(
        claude_home()?
            .join("plugins")
            .join("installed_plugins.json"),
    )?;
    let plugins = doc.get("plugins")?.as_object()?;
    let canon = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    for (key, records) in plugins {
        if !key.starts_with("discord@") {
            continue;
        }
        let Some(records) = records.as_array() else {
            continue;
        };
        let usable = records
            .iter()
            .any(|r| match r.get("scope").and_then(|s| s.as_str()) {
                Some("user") => true,
                _ => {
                    let pp = r.get("projectPath").and_then(|p| p.as_str()).unwrap_or("");
                    !pp.is_empty()
                        && (Path::new(pp) == dir
                            || fs::canonicalize(pp).map(|p| p == canon).unwrap_or(false))
                }
            });
        if usable {
            return Some(key.clone());
        }
    }
    None
}

/// user-scope enabledPlugins — absent means enabled (install sets true;
/// disable writes false).
fn enabled(key: &str) -> bool {
    claude_home()
        .and_then(|h| read_json(h.join("settings.json")))
        .and_then(|d| d.get("enabledPlugins")?.get(key)?.as_bool())
        != Some(false)
}

fn marketplace_known() -> bool {
    claude_home()
        .and_then(|h| read_json(h.join("settings.json")))
        .and_then(|d| {
            d.get("extraKnownMarketplaces")?
                .as_object()
                .map(|o| o.contains_key(MARKETPLACE))
        })
        .unwrap_or(false)
}

pub fn plugin_state(dir: &Path) -> PluginState {
    match usable_install(dir) {
        None => PluginState::Missing,
        Some(key) if enabled(&key) => PluginState::Ready,
        Some(_) => PluginState::Disabled,
    }
}

fn run_claude(args: &[&str]) -> Result<()> {
    let out = Command::new("claude")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .context("failed to run `claude`")?;
    if !out.status.success() {
        let detail = [out.stderr, out.stdout]
            .iter()
            .map(|b| String::from_utf8_lossy(b).trim().to_string())
            .find(|s| !s.is_empty())
            .unwrap_or_default();
        bail!("`claude {}`: {}", args.join(" "), detail);
    }
    Ok(())
}

/// Get the discord channel plugin into Ready state for `dir`: re-enable
/// when disabled, add the marketplace when unknown, install at user
/// scope otherwise. Verifies state afterwards — install exit codes alone
/// are not trustworthy.
pub fn ensure_plugin(dir: &Path) -> Result<()> {
    match plugin_state(dir) {
        PluginState::Ready => return Ok(()),
        PluginState::Disabled => run_claude(&["plugin", "enable", PLUGIN_ID])?,
        PluginState::Missing => {
            if !marketplace_known() {
                run_claude(&["plugin", "marketplace", "add", MARKETPLACE_REPO])?;
            }
            run_claude(&["plugin", "install", PLUGIN_ID, "--scope", "user", "-y"])?;
        }
    }
    if plugin_state(dir) == PluginState::Ready {
        Ok(())
    } else {
        bail!("discord plugin still not usable after install")
    }
}
