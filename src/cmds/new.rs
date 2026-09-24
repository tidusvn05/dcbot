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
- Replying to Discord: inbound messages arrive as `<channel
  source="plugin:discord:discord" chat_id="...">` blocks. Always answer them
  with the `mcp__plugin_discord_discord__reply` tool, passing `chat_id` back —
  transcript text never reaches Discord. If the tool's schema isn't loaded
  yet, call `ToolSearch` with `select:mcp__plugin_discord_discord__reply`
  first, then call it.
- `.claude/settings.json` pre-allows the discord plugin tools, the standard
  toolset (Edit/Write/WebFetch/Agent/…), and read-only shell/git/dcbot
  commands — run them freely. Unusual shell commands (rm, interpreters,
  network) prompt the owner, relayed to their DMs.
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

/// Discord plugin tools the session may call without prompting. The bot runs
/// headless — an approval dialog would stall every reply — and the plugin's
/// own outbound gate already confines these to allowlisted chats.
const DISCORD_TOOL_ALLOW: &[&str] = &[
    "mcp__plugin_discord_discord__reply",
    "mcp__plugin_discord_discord__react",
    "mcp__plugin_discord_discord__edit_message",
    "mcp__plugin_discord_discord__fetch_messages",
    "mcp__plugin_discord_discord__download_attachment",
];

/// Baseline so a fresh deployment works like a normal session without an
/// operator babysitting prompts: the standard toolset (edit/write/web/agent/
/// task tools), read-only shell inspection, text processing, git reads,
/// light file ops, and read-only dcbot subcommands.
///
/// Deliberately excluded — unusual shell commands still prompt (the plugin
/// relays that prompt to the owner's DMs): interpreters (python/node/bun/sh),
/// `find` (-exec/-delete = arbitrary exec), `xargs`, `sed`/`awk`/`tee` (write
/// files), `rm`, network tools, env/printenv (secret leakage), git write ops,
/// and access-mutating dcbot subcommands (allow/remove/policy/approve — a
/// channel message must never be able to change who can reach the bot).
const BASE_TOOL_ALLOW: &[&str] = &[
    // Standard toolset — the everyday tools an interactive session uses
    "Agent",
    "Artifact",
    "AskUserQuestion",
    "BashOutput",
    "CronCreate",
    "CronDelete",
    "CronList",
    "DesignSync",
    "Edit",
    "EnterPlanMode",
    "EnterWorktree",
    "ExitPlanMode",
    "ExitWorktree",
    "KillShell",
    "ListAgents",
    "ListMcpResourcesTool",
    "Monitor",
    "NotebookEdit",
    "PushNotification",
    "ReadMcpResourceDirTool",
    "ReadMcpResourceTool",
    "RemoteTrigger",
    "ReportFindings",
    "ScheduleWakeup",
    "SendFeedback",
    "SendMessage",
    "Skill",
    "TaskCreate",
    "TaskGet",
    "TaskList",
    "TaskOutput",
    "TaskStop",
    "TaskUpdate",
    "TodoWrite",
    "WebFetch",
    "WebSearch",
    "Workflow",
    "Write",
    // Read-only inspection
    "Bash(ls:*)",
    "Bash(cat:*)",
    "Bash(head:*)",
    "Bash(tail:*)",
    "Bash(wc:*)",
    "Bash(file:*)",
    "Bash(stat:*)",
    "Bash(du:*)",
    "Bash(df:*)",
    "Bash(tree:*)",
    "Bash(basename:*)",
    "Bash(dirname:*)",
    "Bash(realpath:*)",
    "Bash(readlink:*)",
    "Bash(pwd:*)",
    "Bash(ps:*)",
    "Bash(uptime:*)",
    "Bash(whoami)",
    "Bash(uname:*)",
    "Bash(date:*)",
    "Bash(hostname)",
    // Text processing (no in-place writes)
    "Bash(grep:*)",
    "Bash(rg:*)",
    "Bash(jq:*)",
    "Bash(sort:*)",
    "Bash(uniq:*)",
    "Bash(cut:*)",
    "Bash(tr:*)",
    "Bash(diff:*)",
    "Bash(comm:*)",
    "Bash(column:*)",
    "Bash(echo:*)",
    "Bash(printf:*)",
    // Git read ops — write ops (add/commit/push/checkout) still prompt
    "Bash(git status:*)",
    "Bash(git diff:*)",
    "Bash(git log:*)",
    "Bash(git show:*)",
    "Bash(git branch:*)",
    "Bash(git tag:*)",
    "Bash(git blame:*)",
    "Bash(git shortlog:*)",
    "Bash(git describe:*)",
    "Bash(git rev-parse:*)",
    "Bash(git ls-files:*)",
    "Bash(git grep:*)",
    "Bash(git stash list:*)",
    "Bash(git config --get:*)",
    "Bash(git remote get-url:*)",
    // Light file ops — no rm
    "Bash(mkdir:*)",
    "Bash(touch:*)",
    "Bash(cp:*)",
    "Bash(mv:*)",
    // dcbot read-only subcommands — access mutations stay gated
    "Bash(dcbot list:*)",
    "Bash(dcbot status:*)",
    "Bash(dcbot doctor:*)",
    "Bash(dcbot logs:*)",
    "Bash(dcbot invite:*)",
    "Bash(dcbot agent)",
];

/// Ensure `.claude/settings.json` auto-allows the discord plugin's tools plus
/// a baseline of common read-only commands. Merges into an existing file —
/// user entries are preserved; rules already present are not duplicated.
pub fn write_settings(dir: &Path) -> Result<()> {
    let path = dir.join(".claude/settings.json");
    let mut doc: serde_json::Value = if path.exists() {
        serde_json::from_str(&fs::read_to_string(&path)?).context("parse .claude/settings.json")?
    } else {
        serde_json::json!({})
    };
    let perms = doc
        .as_object_mut()
        .context(".claude/settings.json is not a JSON object")?
        .entry("permissions")
        .or_insert_with(|| serde_json::json!({}));
    let allow = perms
        .as_object_mut()
        .context(".claude/settings.json: permissions is not an object")?
        .entry("allow")
        .or_insert_with(|| serde_json::json!([]))
        .as_array_mut()
        .context(".claude/settings.json: permissions.allow is not an array")?;
    for rule in DISCORD_TOOL_ALLOW.iter().chain(BASE_TOOL_ALLOW.iter()) {
        if !allow.iter().any(|v| v.as_str() == Some(*rule)) {
            allow.push(serde_json::json!(rule));
        }
    }
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(&path, serde_json::to_string_pretty(&doc)? + "\n")?;
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
    write_settings(&dir)?;

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
