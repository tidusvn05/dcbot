use anyhow::Result;
use console::style;
use rust_i18n::t;

use crate::resolve::resolve;
use crate::state;

fn ok(msg: String) {
    println!("{} {msg}", style("✓").green().bold());
}

pub fn approve(code: &str, name: Option<&str>) -> Result<()> {
    let bot = resolve(name)?;
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
