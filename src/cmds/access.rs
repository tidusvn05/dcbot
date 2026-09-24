use anyhow::{bail, Result};
use console::style;
use rust_i18n::t;
use std::time::{Duration, Instant};

use crate::resolve::{resolve, Resolved};
use crate::state::{self, Access};

fn ok(msg: String) {
    println!("{} {msg}", style("✓").green().bold());
}

/// One display line per pending pairing: code, sender, age, time-to-live.
/// Shared by bare `dcbot approve`/`dcbot pair` and `dcbot status`.
pub fn pending_lines(a: &Access) -> Vec<String> {
    let now = chrono::Utc::now().timestamp_millis();
    a.pending
        .iter()
        .map(|(code, p)| {
            t!(
                "access.pending_line",
                code = code.as_str(),
                sender = p.sender_id.as_str(),
                age = human_secs(now - p.created_at).as_str(),
                ttl = human_secs(p.expires_at - now).as_str()
            )
            .to_string()
        })
        .collect()
}

fn human_secs(ms: i64) -> String {
    let s = ms / 1000;
    if s < 0 {
        "expired".to_string()
    } else if s < 120 {
        format!("{s}s")
    } else {
        format!("{}m", s / 60)
    }
}

/// `dcbot approve [code]` / `dcbot pair [--wait [secs]]`.
/// No code + no --wait → list pending. --wait → auto-approve the next
/// pairing DM that arrives (or a sole existing pending entry).
pub fn approve(code: Option<&str>, wait: Option<u64>, name: Option<&str>) -> Result<()> {
    let bot = resolve(name)?;
    match (code, wait) {
        (Some(code), _) => {
            let sender = state::approve(&bot.state_dir, code)?;
            ok(t!(
                "access.approved",
                sender = sender.as_str(),
                bot = bot.name.as_str()
            )
            .to_string());
            println!("  {}", t!("access.approved_hint"));
            Ok(())
        }
        (None, Some(secs)) => pair_wait(&bot, secs),
        (None, None) => list_pending(&bot),
    }
}

fn list_pending(bot: &Resolved) -> Result<()> {
    let a = state::load(&bot.state_dir)?;
    if a.pending.is_empty() {
        println!("{}", t!("access.no_pending"));
        return Ok(());
    }
    for line in pending_lines(&a) {
        println!("  {line}");
    }
    println!("{}", t!("access.approve_hint"));
    Ok(())
}

/// Poll access.json until a pending pairing appears, then approve it —
/// "DM your bot, I'll handle the code". Existing sole pending is approved
/// immediately; multiple pending bail to explicit `approve <code>` (same
/// reason the plugin skill refuses to auto-pick).
pub fn pair_wait(bot: &Resolved, secs: u64) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(secs);
    println!("{}", t!("access.pair_waiting", secs = secs));
    loop {
        let a = state::load(&bot.state_dir)?;
        let now = chrono::Utc::now().timestamp_millis();
        let live: Vec<&String> = a
            .pending
            .iter()
            .filter(|(_, p)| p.expires_at > now)
            .map(|(c, _)| c)
            .collect();
        match live.len() {
            0 => {
                if Instant::now() >= deadline {
                    bail!(t!("access.pair_timeout", secs = secs));
                }
                std::thread::sleep(Duration::from_secs(1));
            }
            1 => {
                let sender = state::approve(&bot.state_dir, live[0])?;
                ok(t!(
                    "access.approved",
                    sender = sender.as_str(),
                    bot = bot.name.as_str()
                )
                .to_string());
                println!("  {}", t!("access.approved_hint"));
                return Ok(());
            }
            _ => {
                for line in pending_lines(&a) {
                    println!("  {line}");
                }
                bail!(t!("access.pair_multi"));
            }
        }
    }
}

pub fn deny(code: &str, name: Option<&str>) -> Result<()> {
    let bot = resolve(name)?;
    let sender = state::deny(&bot.state_dir, code)?;
    ok(t!("access.denied", sender = sender.as_str(), code = code).to_string());
    Ok(())
}

pub fn allow(user_id: &str, name: Option<&str>) -> Result<()> {
    let bot = resolve(name)?;
    if state::allow(&bot.state_dir, user_id)? {
        ok(t!("access.allowed", id = user_id, bot = bot.name.as_str()).to_string());
    } else {
        println!("{}", t!("access.already_allowed", id = user_id));
    }
    Ok(())
}

pub fn remove(user_id: &str, name: Option<&str>) -> Result<()> {
    let bot = resolve(name)?;
    if state::remove(&bot.state_dir, user_id)? {
        ok(t!("access.removed", id = user_id, bot = bot.name.as_str()).to_string());
    } else {
        println!("{}", t!("access.not_in_list", id = user_id));
    }
    Ok(())
}

pub fn policy(mode: &str, name: Option<&str>) -> Result<()> {
    let bot = resolve(name)?;
    state::set_policy(&bot.state_dir, mode)?;
    ok(t!("access.policy_set", mode = mode, bot = bot.name.as_str()).to_string());
    if mode == "pairing" {
        println!("  {}", t!("access.pairing_warn"));
    }
    Ok(())
}

pub fn group_add(
    channel_id: &str,
    no_mention: bool,
    allow: Option<&str>,
    name: Option<&str>,
) -> Result<()> {
    let bot = resolve(name)?;
    let allow_from: Vec<String> = allow
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_default();
    state::group_add(&bot.state_dir, channel_id, !no_mention, allow_from)?;
    ok(t!(
        "access.group_added",
        id = channel_id,
        bot = bot.name.as_str()
    )
    .to_string());
    Ok(())
}

pub fn group_rm(channel_id: &str, name: Option<&str>) -> Result<()> {
    let bot = resolve(name)?;
    if state::group_rm(&bot.state_dir, channel_id)? {
        ok(t!("access.group_removed", id = channel_id).to_string());
    } else {
        println!("{}", t!("access.group_not_found", id = channel_id));
    }
    Ok(())
}

pub fn set(key: &str, value: &str, name: Option<&str>) -> Result<()> {
    let bot = resolve(name)?;
    state::set_key(&bot.state_dir, key, value)?;
    ok(t!("access.set_ok", key = key, bot = bot.name.as_str()).to_string());
    Ok(())
}
