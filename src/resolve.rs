use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

use crate::manifest::{self, Manifest};
use crate::registry::Registry;

pub struct Resolved {
    pub name: String,
    pub dir: PathBuf,
    pub state_dir: PathBuf,
    pub manifest: Option<Manifest>,
}

/// Resolve a bot by name (registry) or, when name is None, by walking up
/// from cwd looking for a `.discord-state/` dir.
pub fn resolve(name: Option<&str>) -> Result<Resolved> {
    match name {
        Some(n) => {
            let reg = Registry::load();
            let entry = reg
                .bots
                .get(n)
                .with_context(|| format!("unknown bot '{n}' — run `dcbot list`"))?;
            from_parts(n.to_string(), entry.dir.clone())
        }
        None => {
            let cwd = std::env::current_dir()?;
            let dir = find_state_upward(&cwd).with_context(|| {
                "not inside a bot deployment (no .discord-state upward) — pass a name or cd into a deployment"
            })?;
            let reg = Registry::load();
            let name = reg.name_for_dir(&dir).unwrap_or_else(|| {
                dir.file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "bot".to_string())
            });
            from_parts(name, dir)
        }
    }
}

fn from_parts(name: String, dir: PathBuf) -> Result<Resolved> {
    if !dir.is_dir() {
        bail!(
            "deployment dir missing: {} — run `dcbot prune` or `dcbot forget`",
            dir.display()
        );
    }
    let state_dir = dir.join(".discord-state");
    let manifest = manifest::load(&dir);
    Ok(Resolved {
        name,
        dir,
        state_dir,
        manifest,
    })
}

/// Walk up from `start` looking for a directory containing `.discord-state/`.
fn find_state_upward(start: &Path) -> Option<PathBuf> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        if d.join(".discord-state").is_dir() {
            return Some(d.to_path_buf());
        }
        dir = d.parent();
    }
    None
}
