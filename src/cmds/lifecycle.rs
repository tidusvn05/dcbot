use anyhow::{bail, Context, Result};
use console::style;
use rust_i18n::t;
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::cmds::new::RUN_SH;
use crate::resolve::resolve;
use crate::tmux;

pub fn start(name: &str, respawn: bool) -> Result<()> {
    if !tmux::installed() {
        bail!(t!("lifecycle.tmux_missing"));
    }
    let bot = resolve(Some(name))?;
    let run_sh = bot.dir.join("run.sh");
    if !run_sh.exists() {
        // Self-heal: manifest exists but run.sh was deleted.
        fs::write(&run_sh, RUN_SH)?;
        fs::set_permissions(&run_sh, fs::Permissions::from_mode(0o755))?;
    }
    let session = tmux::session_name(&bot.name);
    if tmux::exists(&session) {
        bail!(t!("lifecycle.already_running", name = bot.name.as_str()));
    }
    if let Err(e) = ensure_trusted(&bot.dir) {
        eprintln!(
            "{} {}",
            style(t!("common.warn")).yellow().bold(),
            t!("lifecycle.trust_failed", err = e.to_string())
        );
    }
    tmux::start(&session, &bot.dir, respawn)?;
    println!(
        "{} {}",
        style("✓").green().bold(),
        t!(
            "lifecycle.started",
            name = bot.name.as_str(),
            session = session.as_str()
        )
    );
    println!(
        "  {}",
        t!("lifecycle.attach_hint", name = bot.name.as_str())
    );
    Ok(())
}

/// Pre-accept Claude Code's workspace-trust dialog for the deployment dir
/// (`projects[dir].hasTrustDialogAccepted` in ~/.claude.json) — otherwise
/// the first tmux launch sits on a prompt nobody can answer.
fn ensure_trusted(dir: &Path) -> Result<()> {
    let cfg = dirs::home_dir()
        .map(|h| h.join(".claude.json"))
        .context("no home dir")?;
    let mode = fs::metadata(&cfg).ok().map(|m| m.permissions().mode());
    let mut doc: serde_json::Value = match fs::read_to_string(&cfg) {
        Ok(raw) => serde_json::from_str(&raw).context("~/.claude.json is not valid JSON")?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => json!({}),
        Err(e) => return Err(e).context("reading ~/.claude.json"),
    };
    if !doc.is_object() {
        bail!("~/.claude.json is not a JSON object");
    }

    // Claude keys projects by launch cwd — cover both the path as given
    // and its canonical form (symlinked parents differ).
    let mut keys = vec![dir.to_string_lossy().to_string()];
    let canon = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    if canon != dir {
        keys.push(canon.to_string_lossy().to_string());
    }

    let projects = doc
        .as_object_mut()
        .unwrap()
        .entry("projects")
        .or_insert_with(|| json!({}));
    if !projects.is_object() {
        bail!("~/.claude.json: 'projects' is not an object");
    }
    let projects = projects.as_object_mut().unwrap();

    let mut dirty = false;
    for key in keys {
        let proj = projects.entry(key).or_insert_with(|| json!({}));
        if let Some(p) = proj.as_object_mut() {
            if p.get("hasTrustDialogAccepted") != Some(&json!(true)) {
                p.insert("hasTrustDialogAccepted".into(), json!(true));
                dirty = true;
            }
        }
    }
    if !dirty {
        return Ok(());
    }

    // Atomic write; keep the file's original mode (claude writes 0600).
    let tmp = cfg.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string(&doc)?)?;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(mode.unwrap_or(0o600)))?;
    fs::rename(&tmp, &cfg)?;
    Ok(())
}

pub fn stop(name: &str) -> Result<()> {
    let bot = resolve(Some(name))?;
    let session = tmux::session_name(&bot.name);
    tmux::stop(&session)?;
    println!(
        "{} {}",
        style("✓").green().bold(),
        t!("lifecycle.stopped", name = bot.name.as_str())
    );
    Ok(())
}

pub fn restart(name: &str, respawn: bool) -> Result<()> {
    let bot = resolve(Some(name))?;
    let session = tmux::session_name(&bot.name);
    if tmux::exists(&session) {
        tmux::stop(&session)?;
    }
    start(&bot.name, respawn)
}

pub fn attach(name: &str) -> Result<()> {
    let bot = resolve(Some(name))?;
    let session = tmux::session_name(&bot.name);
    if !tmux::exists(&session) {
        bail!(t!("lifecycle.not_running", name = bot.name.as_str()));
    }
    let code = tmux::attach(&session)?;
    if code != 0 {
        bail!("tmux attach exited with {code}");
    }
    Ok(())
}

pub fn logs(name: &str, lines: u32, follow: bool) -> Result<()> {
    let bot = resolve(Some(name))?;
    let session = tmux::session_name(&bot.name);
    if !tmux::exists(&session) {
        bail!(t!("lifecycle.not_running", name = bot.name.as_str()));
    }
    let mut printed = 0usize;
    loop {
        let body = tmux::capture(&session, lines)?;
        let all: Vec<&str> = body.lines().collect();
        let fresh = if follow && all.len() > printed {
            &all[printed..]
        } else if !follow {
            &all[..]
        } else {
            &[][..]
        };
        for l in fresh {
            println!("{l}");
        }
        printed = all.len();
        if !follow {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}
