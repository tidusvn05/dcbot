use anyhow::{bail, Context, Result};
use console::style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Password};
use rust_i18n::t;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use crate::cmds::lifecycle;
use crate::discord;
use crate::manifest::{self, Manifest};
use crate::registry::{Entry, Registry};
use crate::state::{self, Access};

pub const RUN_SH: &str = r#"#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")"
export DISCORD_STATE_DIR="$PWD/.discord-state"

if [[ "${1:-}" == "--respawn" ]]; then
  while true; do
    claude --channels plugin:discord@claude-plugins-official "$@" || true
    echo "[run.sh] claude exited — restarting in 5s (Ctrl-C to stop)"
    sleep 5
  done
else
  exec claude --channels plugin:discord@claude-plugins-official "$@"
fi
"#;

pub fn run(name: &str, dir: Option<PathBuf>, here: bool) -> Result<()> {
    let theme = ColorfulTheme::default();

    // Prereqs — warn but don't block; the user may install them later.
    let missing: Vec<&str> = ["claude", "bun", "tmux"]
        .iter()
        .filter(|t| which::which(t).is_err())
        .copied()
        .collect();
    if !missing.is_empty() {
        eprintln!(
            "{} {}",
            style(t!("common.warn")).yellow().bold(),
            t!("new.prereq_missing", tools = missing.join(", "))
        );
    }

    let dir = if here {
        std::env::current_dir()?
    } else {
        dir.unwrap_or_else(|| std::env::current_dir().unwrap().join(name))
    };
    if dir.join(".discord-state").exists() {
        bail!(t!("new.dir_exists", dir = dir.display()));
    }
    if Registry::load().bots.contains_key(name) {
        bail!(t!("new.name_taken", name = name));
    }

    println!("{}", t!("new.guide"));
    let _ = Confirm::with_theme(&theme)
        .with_prompt(t!("new.guide_ready").to_string())
        .default(true)
        .interact()?;

    // Token → validate against Discord → confirm identity.
    let token = Password::with_theme(&theme)
        .with_prompt(t!("new.token_prompt").to_string())
        .interact()?;
    let bot = discord::fetch_me(token.trim())?;
    println!(
        "{}",
        t!("new.token_ok", tag = bot.tag(), id = bot.id.as_str())
    );

    // Owner snowflake — empty keeps pairing mode.
    let owner = Input::<String>::with_theme(&theme)
        .with_prompt(t!("new.owner_prompt").to_string())
        .allow_empty(true)
        .interact_text()?;
    let owner = owner.trim().to_string();
    if !owner.is_empty() && !is_snowflake(&owner) {
        bail!(t!("new.owner_invalid", id = owner.as_str()));
    }

    // --- Write the deployment -------------------------------------------------
    let state_dir = dir.join(".discord-state");
    fs::create_dir_all(state_dir.join("approved"))?;
    fs::create_dir_all(state_dir.join("inbox"))?;
    fs::create_dir_all(dir.join("logs"))?;

    let env_file = state_dir.join(".env");
    fs::write(&env_file, format!("DISCORD_BOT_TOKEN={}\n", token.trim()))?;
    fs::set_permissions(&env_file, fs::Permissions::from_mode(0o600))?;

    let mut access = Access::default();
    if !owner.is_empty() {
        access.dm_policy = "allowlist".to_string();
        access.allow_from.push(owner);
    }
    state::save(&state_dir, &access)?;

    let manifest = Manifest {
        name: name.to_string(),
        bot_user_id: bot.id.clone(),
        bot_tag: bot.tag(),
        created_at: chrono::Utc::now().to_rfc3339(),
        channels_flag: manifest::default_channels_flag(),
        autorestart: false,
    };
    manifest::save(&dir, &manifest)?;

    let run_sh = dir.join("run.sh");
    fs::write(&run_sh, RUN_SH)?;
    fs::set_permissions(&run_sh, fs::Permissions::from_mode(0o755))?;

    let gi = dir.join(".gitignore");
    let gi_body = fs::read_to_string(&gi).unwrap_or_default();
    if !gi_body.lines().any(|l| l.trim() == ".discord-state/") {
        fs::write(&gi, format!("{gi_body}.discord-state/\n"))?;
    }

    // --- Register -------------------------------------------------------------
    let mut reg = Registry::load();
    reg.add(
        name,
        Entry {
            dir: std::fs::canonicalize(&dir).unwrap_or(dir.clone()),
            bot_user_id: bot.id.clone(),
            bot_tag: bot.tag(),
            created_at: manifest.created_at.clone(),
        },
    );
    reg.save()?;

    println!(
        "\n{} {}",
        style("✓").green().bold(),
        t!("new.created", dir = dir.display())
    );

    if Confirm::with_theme(&theme)
        .with_prompt(t!("new.start_now").to_string())
        .default(true)
        .interact()?
    {
        lifecycle::start(name, false).context("start failed")?;
    } else {
        println!("{}", t!("new.start_hint", name = name));
    }
    Ok(())
}

fn is_snowflake(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii_digit()) && (17..=20).contains(&s.len())
}
