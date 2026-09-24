use anyhow::{bail, Result};
use console::style;
use rust_i18n::t;

use crate::resolve::resolve;
use crate::{discord, state};

/// DM an allowlisted user straight through the Discord API — same path the
/// start-greeting uses. Works even when the session has no inbound message
/// (the plugin's reply tool only learns the DM channel id from one).
pub fn send(text: &str, to: Option<&str>, name: Option<&str>) -> Result<()> {
    let bot = resolve(name)?;
    let access = state::load(&bot.state_dir)?;
    let recipient = match to {
        Some(id) => {
            if !access.allow_from.iter().any(|u| u == id) {
                bail!(t!("dm.not_allowed", id = id, bot = bot.name.as_str()).to_string());
            }
            id.to_string()
        }
        None => access
            .allow_from
            .first()
            .cloned()
            .ok_or_else(|| anyhow::anyhow!(t!("dm.no_recipient").to_string()))?,
    };
    let Some(token) = state::read_token(&bot.state_dir) else {
        bail!(t!("dm.no_token"));
    };
    let channel = discord::open_dm(&token, &recipient)?;
    discord::send_message(&token, &channel, text)?;
    println!(
        "{} {}",
        style("✓").green().bold(),
        t!(
            "dm.sent",
            user = recipient.as_str(),
            bot = bot.name.as_str()
        )
    );
    Ok(())
}
