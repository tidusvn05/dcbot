use anyhow::{bail, Context, Result};
use console::style;
use rust_i18n::t;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use crate::discord;
use crate::manifest::{self, Manifest};
use crate::registry::{Entry, Registry};
use crate::state;
use crate::tmux;

/// Adopt an existing deployment dir into the registry.
pub fn register(dir: PathBuf) -> Result<()> {
    let dir = std::fs::canonicalize(&dir).context("deployment dir not found")?;
    let state_dir = dir.join(".discord-state");
    if !state_dir.is_dir() {
        bail!(t!("reg.no_state", dir = dir.display()));
    }

    // Prefer existing manifest; otherwise derive identity from the token.
    let name;
    let (bot_user_id, bot_tag, created_at);
    if let Some(m) = manifest::load(&dir) {
        name = m.name.clone();
        bot_user_id = m.bot_user_id;
        bot_tag = m.bot_tag;
        created_at = m.created_at;
    } else {
        let token = state::read_token(&state_dir)
            .context("no bot.toml and no token in .env — cannot identify bot")?;
        let bot = discord::fetch_me(&token)?;
        name = dir
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "bot".to_string());
        bot_tag = bot.tag();
        bot_user_id = bot.id;
        created_at = chrono::Utc::now().to_rfc3339();
        manifest::save(
            &dir,
            &Manifest {
                name: name.clone(),
                bot_user_id: bot_user_id.clone(),
                bot_tag: bot_tag.clone(),
                app_id: discord::fetch_application(&token).ok().map(|a| a.id),
                created_at: created_at.clone(),
                channels_flag: manifest::default_channels_flag(),
                autorestart: false,
            },
        )?;
    }

    // Make the deployment runnable immediately — same self-heal as `start`.
    let run_sh = dir.join("run.sh");
    if !run_sh.exists() {
        fs::write(&run_sh, crate::cmds::new::RUN_SH)?;
        fs::set_permissions(&run_sh, fs::Permissions::from_mode(0o755))?;
    }
    fs::create_dir_all(dir.join("logs"))?;

    let mut reg = Registry::load();
    if reg.bots.contains_key(&name) {
        bail!(t!("new.name_taken", name = name.as_str()));
    }
    reg.add(
        &name,
        Entry {
            dir,
            bot_user_id,
            bot_tag,
            created_at,
        },
    );
    reg.save()?;
    println!(
        "{} {}",
        style("✓").green().bold(),
        t!("reg.registered", name = name.as_str())
    );
    Ok(())
}

pub fn forget(name: &str) -> Result<()> {
    let mut reg = Registry::load();
    if reg.remove(name).is_none() {
        bail!(t!("reg.not_found", name = name));
    }
    reg.save()?;
    println!(
        "{} {}",
        style("✓").green().bold(),
        t!("reg.forgotten", name = name)
    );
    Ok(())
}

/// Drop registry entries whose dirs are gone; report orphan tmux sessions.
pub fn prune() -> Result<()> {
    let mut reg = Registry::load();
    let stale: Vec<String> = reg
        .bots
        .iter()
        .filter(|(_, e)| !e.dir.is_dir())
        .map(|(n, _)| n.clone())
        .collect();
    for n in &stale {
        reg.remove(n);
        println!(
            "{} {}",
            style("✓").green().bold(),
            t!("reg.pruned", name = n.as_str())
        );
    }
    reg.save()?;
    if stale.is_empty() {
        println!("{}", t!("reg.nothing_stale"));
    }
    for s in tmux::sessions() {
        let bot = s.trim_start_matches("dcbot-");
        if !reg.bots.contains_key(bot) {
            eprintln!(
                "{} {}",
                style(t!("common.warn")).yellow().bold(),
                t!("list.warn_orphan", session = s.as_str())
            );
        }
    }
    Ok(())
}
