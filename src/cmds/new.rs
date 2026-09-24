use anyhow::{bail, Context, Result};
use console::style;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Password};
use rust_i18n::t;
use std::fs;
use std::io::IsTerminal;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::cmds::lifecycle;
use crate::discord;
use crate::manifest::{self, Manifest};
use crate::registry::{Entry, Registry};
use crate::state::{self, Access};
use crate::util;

pub struct NewOpts {
    pub name: String,
    pub dir: Option<PathBuf>,
    pub here: bool,
    pub token: Option<String>,
    pub owner: Option<String>,
    pub yes: bool,
    pub start: bool,
    pub pair: bool,
}

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

/// Session rule file — Claude Code auto-loads `.claude/rules/*.md` at launch
/// (same priority as CLAUDE.md). Teaches the session that access control goes
/// through `dcbot`, never the plugin's `/discord:*` skills (they hardcode the
/// global state dir and would silently edit the wrong files).
pub const DCBOT_RULE: &str = r#"This directory is a **dcbot** deployment — a Discord-channel bot for Claude
Code with its own `.discord-state/`. Operate it with the `dcbot` CLI, never
the plugin's built-in skills.

- Never run `/discord:access` or `/discord:configure` — both hardcode the
  global `~/.claude/channels/discord` dir and would silently edit files this
  server never reads. dcbot equivalents: `dcbot approve <code>` ·
  `dcbot pair --wait` · `deny` · `allow` · `remove` · `policy` ·
  `group add|rm` · `set` · `status`.
- Pairing: when the bot replies "Pairing required", approve from this dir —
  `dcbot approve <code>` (the bot name resolves from `.discord-state`).
- Proactive DM to an allowlisted user: `dcbot dm <text> [--to <snowflake>]`
  — works without an inbound message.
- Don't hand-edit `.discord-state/` — use the CLI so validation and the
  `approved/` marker stay correct.
- Session lifecycle (`dcbot start|stop|restart|logs|attach`) belongs to the
  user's terminal — never launch `claude --channels` yourself.
- Full usage contract: `dcbot agent`.
"#;

/// Write `.claude/rules/dcbot.md` into a deployment dir — write-if-missing
/// so user edits are never clobbered.
pub fn write_rule(dir: &Path) -> Result<()> {
    let rule = dir.join(".claude/rules/dcbot.md");
    if rule.exists() {
        return Ok(());
    }
    fs::create_dir_all(rule.parent().unwrap())?;
    fs::write(&rule, DCBOT_RULE)?;
    Ok(())
}

pub fn run(opts: NewOpts) -> Result<()> {
    let env_token = std::env::var("DCBOT_BOT_TOKEN")
        .ok()
        .filter(|t| !t.is_empty());
    let noninteractive = opts.yes || opts.token.is_some() || env_token.is_some();
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

    let dir = if opts.here {
        std::env::current_dir()?
    } else {
        opts.dir
            .unwrap_or_else(|| std::env::current_dir().unwrap().join(&opts.name))
    };
    if dir.join(".discord-state").exists() {
        bail!(t!("new.dir_exists", dir = dir.display()));
    }
    if Registry::load().bots.contains_key(&opts.name) {
        bail!(t!("new.name_taken", name = opts.name.as_str()));
    }

    if !noninteractive {
        println!("{}", t!("new.guide"));
        let _ = Confirm::with_theme(&theme)
            .with_prompt(t!("new.guide_ready").to_string())
            .default(true)
            .interact()?;
    }

    // Token → validate against Discord → confirm identity.
    let token = match opts.token.or(env_token) {
        Some(tok) => tok.trim().to_string(),
        None => {
            // --yes under an agent (no TTY) can't prompt — bail clearly.
            // In a real terminal, --yes still asks for the token only.
            if noninteractive && !std::io::stdin().is_terminal() {
                bail!(t!("new.token_required"));
            }
            Password::with_theme(&theme)
                .with_prompt(t!("new.token_prompt").to_string())
                .interact()?
                .trim()
                .to_string()
        }
    };
    let bot = discord::fetch_me(&token)?;
    println!(
        "{}",
        t!("new.token_ok", tag = bot.tag(), id = bot.id.as_str())
    );

    // Application metadata → invite URL + Message Content Intent check.
    // Best-effort: on failure the bot user id doubles as the client_id.
    let app = discord::fetch_application(&token).ok();
    let client_id = app
        .as_ref()
        .map(|a| a.id.as_str())
        .unwrap_or(bot.id.as_str());
    let url = discord::invite_url(client_id);
    println!(
        "{}",
        t!(
            "new.invite_url",
            url = url.as_str(),
            name = opts.name.as_str()
        )
    );
    if app.as_ref().and_then(|a| a.message_content_intent()) == Some(false) {
        eprintln!(
            "{} {}",
            style(t!("common.warn")).yellow().bold(),
            t!("new.intent_missing")
        );
    }
    if !noninteractive
        && Confirm::with_theme(&theme)
            .with_prompt(t!("new.open_invite").to_string())
            .default(true)
            .interact()?
    {
        if let Err(e) = util::open_browser(&url) {
            eprintln!(
                "{} {}",
                style(t!("common.warn")).yellow().bold(),
                t!("invite.no_open", err = e.to_string())
            );
        }
    }

    // Discord channel plugin — auto-install so `dcbot start` never lands
    // on "plugin not installed" inside the tmux session.
    let pstate = crate::claude::plugin_state(&dir);
    if pstate != crate::claude::PluginState::Ready {
        println!(
            "{}",
            match pstate {
                crate::claude::PluginState::Disabled => t!("plugin.enabling"),
                _ => t!("plugin.installing"),
            }
        );
        match crate::claude::ensure_plugin(&dir) {
            Ok(()) => println!("{} {}", style("✓").green().bold(), t!("plugin.installed")),
            Err(e) => eprintln!(
                "{} {}",
                style(t!("common.warn")).yellow().bold(),
                t!("plugin.failed", err = e.to_string())
            ),
        }
    }

    // Owner snowflake — empty keeps pairing mode.
    let owner = match opts.owner {
        Some(o) => o.trim().to_string(),
        None if noninteractive => String::new(),
        None => Input::<String>::with_theme(&theme)
            .with_prompt(t!("new.owner_prompt").to_string())
            .allow_empty(true)
            .interact_text()?
            .trim()
            .to_string(),
    };
    if !owner.is_empty() && !is_snowflake(&owner) {
        bail!(t!("new.owner_invalid", id = owner.as_str()));
    }
    let pairing_mode = owner.is_empty();

    // --- Write the deployment -------------------------------------------------
    let state_dir = dir.join(".discord-state");
    fs::create_dir_all(state_dir.join("approved"))?;
    fs::create_dir_all(state_dir.join("inbox"))?;
    fs::create_dir_all(dir.join("logs"))?;

    let env_file = state_dir.join(".env");
    fs::write(&env_file, format!("DISCORD_BOT_TOKEN={token}\n"))?;
    fs::set_permissions(&env_file, fs::Permissions::from_mode(0o600))?;

    let mut access = Access::default();
    if !owner.is_empty() {
        access.dm_policy = "allowlist".to_string();
        access.allow_from.push(owner);
    }
    state::save(&state_dir, &access)?;

    let manifest = Manifest {
        name: opts.name.clone(),
        bot_user_id: bot.id.clone(),
        bot_tag: bot.tag(),
        app_id: Some(client_id.to_string()),
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
    write_rule(&dir)?;

    // --- Register -------------------------------------------------------------
    let mut reg = Registry::load();
    reg.add(
        &opts.name,
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

    let do_start = opts.start
        || (!noninteractive
            && Confirm::with_theme(&theme)
                .with_prompt(t!("new.start_now").to_string())
                .default(true)
                .interact()?);
    if do_start {
        lifecycle::start(&opts.name, false).context("start failed")?;
    } else {
        println!("{}", t!("new.start_hint", name = opts.name.as_str()));
    }

    // Pairing mode (no owner seeded): offer to auto-approve the first DM's
    // pairing code — only works once the session/server is actually up.
    if pairing_mode {
        if do_start {
            let wait = opts.pair
                || (!noninteractive
                    && Confirm::with_theme(&theme)
                        .with_prompt(t!("new.autopair_prompt").to_string())
                        .default(true)
                        .interact()?);
            if wait {
                let bot = crate::resolve::resolve(Some(&opts.name))?;
                if let Err(e) = crate::cmds::access::pair_wait(&bot, 60) {
                    eprintln!("{} {e}", style(t!("common.warn")).yellow().bold(),);
                }
            }
        } else {
            println!("{}", t!("new.pair_hint", name = opts.name.as_str()));
        }
    } else if opts.pair {
        eprintln!(
            "{} {}",
            style(t!("common.warn")).yellow().bold(),
            t!("new.pair_skipped")
        );
    }
    Ok(())
}

fn is_snowflake(s: &str) -> bool {
    s.chars().all(|c| c.is_ascii_digit()) && (17..=20).contains(&s.len())
}
