use anyhow::{bail, Context, Result};
use console::style;
use rust_i18n::t;
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::resolve::{resolve, Resolved};
use crate::tmux;
use crate::{discord, state};

pub fn start(name: &str, respawn: bool) -> Result<()> {
    if !tmux::installed() {
        bail!(t!("lifecycle.tmux_missing"));
    }
    let bot = resolve(Some(name))?;
    // Self-heal: manifest exists but run.sh was deleted — and upgrade
    // v1 deployments to the PATH-hardened template.
    crate::cmds::new::write_run_sh(&bot.dir)?;
    // Same for the session rule file — deployments created before it existed.
    crate::cmds::new::write_rule(&bot.dir)?;
    crate::cmds::new::write_settings(&bot.dir)?;
    let session = tmux::session_name(&bot.name);
    if tmux::exists(&session) {
        // A pane that fell back to a shell after run.sh died still counts
        // as "running" to has-session — but nothing listens on Discord.
        // Recycle it instead of bouncing off already_running forever.
        match tmux::pane_state(&session) {
            tmux::PaneState::DeadShell | tmux::PaneState::Gone => {
                eprintln!(
                    "{} {}",
                    style(t!("common.warn")).yellow().bold(),
                    t!("lifecycle.stale_recycled", session = session.as_str())
                );
                let _ = tmux::kill(&session);
                log_event(&bot.dir, "recycled stale session pane");
            }
            _ => bail!(t!("lifecycle.already_running", name = bot.name.as_str())),
        }
    }
    if let Err(e) = ensure_trusted(&bot.dir) {
        eprintln!(
            "{} {}",
            style(t!("common.warn")).yellow().bold(),
            t!("lifecycle.trust_failed", err = e.to_string())
        );
    }
    // The channel plugin is a hard requirement — a started session without
    // it just idles on "plugin not installed". Auto-install, bail loudly
    // if that fails.
    let pstate = crate::claude::plugin_state(&bot.dir);
    if pstate != crate::claude::PluginState::Ready {
        println!(
            "{}",
            match pstate {
                crate::claude::PluginState::Disabled => t!("plugin.enabling"),
                _ => t!("plugin.installing"),
            }
        );
        if let Err(e) = crate::claude::ensure_plugin(&bot.dir) {
            bail!(t!("plugin.failed", err = e.to_string()));
        }
        println!("{} {}", style("✓").green().bold(), t!("plugin.installed"));
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
    log_event(&bot.dir, "session spawned");
    println!("{}", t!("lifecycle.verifying"));
    match verify_live(&session) {
        Live::Connected(line) => {
            println!(
                "{} {}",
                style("✓").green().bold(),
                t!("lifecycle.live", line = line.as_str())
            );
            log_event(&bot.dir, &format!("live: {line}"));
            greet(&bot);
        }
        Live::Degraded => {
            // claude is up but the gateway never announced itself — the
            // bot may still come online, but don't DM a false "online".
            eprintln!(
                "{} {}",
                style(t!("common.warn")).yellow().bold(),
                t!(
                    "lifecycle.live_degraded",
                    secs = live_timeout().as_secs(),
                    name = bot.name.as_str()
                )
            );
            log_event(&bot.dir, "degraded: gateway never confirmed");
        }
        Live::Dead(reason) => {
            let log = save_failed_capture(&bot.dir, &session);
            log_event(&bot.dir, &format!("dead: {reason}"));
            bail!(t!(
                "lifecycle.live_failed",
                reason = reason.as_str(),
                log = log.display().to_string().as_str()
            ));
        }
    }
    Ok(())
}

enum Live {
    /// The channel server reported "gateway connected as …".
    Connected(String),
    /// claude is still running past the timeout but the gateway never
    /// announced itself — session exists, greeting withheld.
    Degraded,
    /// The pane died or never reached claude.
    Dead(String),
}

/// How long to wait for the Discord gateway before giving up. First
/// boots can be slow (plugin deps, cold model start) — 45s default,
/// overridable for tests via DCBOT_LIVE_TIMEOUT_SECS.
fn live_timeout() -> std::time::Duration {
    std::env::var("DCBOT_LIVE_TIMEOUT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .map(std::time::Duration::from_secs)
        .unwrap_or(std::time::Duration::from_secs(45))
}

/// Poll the pane until the channel server announces the Discord gateway,
/// the pane demonstrably dies, or the timeout hits. A tmux session that
/// merely "exists" is not proof — a dead run.sh still holds the pane
/// open on a fallback shell.
fn verify_live(session: &str) -> Live {
    let deadline = std::time::Instant::now() + live_timeout();
    loop {
        let state = tmux::pane_state(session);
        match &state {
            tmux::PaneState::Gone => {
                return Live::Dead(t!("lifecycle.died").to_string());
            }
            tmux::PaneState::DeadShell => {
                return Live::Dead(pane_diagnosis(session));
            }
            _ => {}
        }
        if let Some(line) = tmux::channel_status(session) {
            return Live::Connected(line);
        }
        if std::time::Instant::now() >= deadline {
            return match state {
                tmux::PaneState::Claude | tmux::PaneState::Other(_) => Live::Degraded,
                _ => Live::Dead(t!("lifecycle.never_claude").to_string()),
            };
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
}

/// Pull the likely failure reason out of a dead pane's tail — the
/// command-not-found / error lines — so `start` can fail with something
/// actionable instead of a bare "died".
fn pane_diagnosis(session: &str) -> String {
    let Ok(body) = tmux::capture(session, 60) else {
        return t!("lifecycle.dead_shell").to_string();
    };
    let hits: Vec<&str> = body
        .lines()
        .filter(|l| {
            let l = l.trim();
            !l.is_empty()
                && (l.contains("command not found")
                    || l.contains("[run.sh]")
                    || l.to_lowercase().contains("error"))
        })
        .collect();
    if hits.is_empty() {
        t!("lifecycle.dead_shell").to_string()
    } else {
        hits[hits.len().saturating_sub(3)..].join(" | ")
    }
}

/// `logs/failed-start-<ts>.log` — full pane scrollback, for post-mortem.
fn save_failed_capture(dir: &Path, session: &str) -> PathBuf {
    let log = dir.join("logs").join(format!(
        "failed-start-{}.log",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    ));
    if let Ok(body) = tmux::capture(session, 10000) {
        let _ = fs::create_dir_all(log.parent().unwrap());
        let _ = fs::write(&log, body);
    }
    log
}

/// `logs/lifecycle.log` — append-only event trail so a silently-dead
/// session is debuggable after the fact, not just in live scrollback.
fn log_event(dir: &Path, msg: &str) {
    use std::io::Write;
    let path = dir.join("logs").join("lifecycle.log");
    let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open(&path) else {
        return;
    };
    let _ = writeln!(
        f,
        "{} {msg}",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );
}

/// Announce that the bot is up: DM the first allowlisted user and post to
/// every configured guild channel — proves the token works end-to-end and
/// gives the owner a channel to reply in. Skipped in pairing mode with no
/// groups (nobody to greet). Best-effort: the session is already running,
/// failures only warn.
fn greet(bot: &Resolved) {
    let Ok(access) = state::load(&bot.state_dir) else {
        return;
    };
    let Some(token) = state::read_token(&bot.state_dir) else {
        return;
    };
    let session = tmux::session_name(&bot.name);
    let text = t!(
        "lifecycle.greeting_text",
        name = bot.name.as_str(),
        session = session.as_str(),
        dir = bot.dir.display().to_string().as_str()
    );
    if let Some(owner) = access.allow_from.first() {
        match discord::open_dm(&token, owner)
            .and_then(|ch| discord::send_message(&token, &ch, &text))
        {
            Ok(()) => println!(
                "{} {}",
                style("✓").green().bold(),
                t!("lifecycle.greeted", owner = owner.as_str())
            ),
            Err(e) => eprintln!(
                "{} {}",
                style(t!("common.warn")).yellow().bold(),
                t!("lifecycle.greet_failed", err = e.to_string())
            ),
        }
    }
    for channel in access.groups.keys() {
        match discord::send_message(&token, channel, &text) {
            Ok(()) => println!(
                "{} {}",
                style("✓").green().bold(),
                t!("lifecycle.greeted_channel", channel = channel.as_str())
            ),
            Err(e) => eprintln!(
                "{} {}",
                style(t!("common.warn")).yellow().bold(),
                t!(
                    "lifecycle.greet_channel_failed",
                    channel = channel.as_str(),
                    err = e.to_string()
                )
            ),
        }
    }
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
    log_event(&bot.dir, "stopped");
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
