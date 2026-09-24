use anyhow::{bail, Result};
use console::style;
use rust_i18n::t;

use crate::discord;
use crate::registry::Registry;
use crate::resolve::resolve;
use crate::util;

/// Print the OAuth2 invite URL — client_id from bot.toml, falling back to
/// the bot user id (identical on standard Discord apps) in the registry.
/// `--open` also launches a browser; `--copy` puts the URL on the clipboard.
pub fn run(name: Option<&str>, open: bool, copy: bool) -> Result<()> {
    let bot = resolve(name)?;
    let client_id = bot
        .manifest
        .as_ref()
        .map(|m| m.app_id.clone().unwrap_or_else(|| m.bot_user_id.clone()))
        .or_else(|| {
            Registry::load()
                .bots
                .get(&bot.name)
                .map(|e| e.bot_user_id.clone())
        });
    let Some(id) = client_id else {
        bail!(t!("invite.no_id", name = bot.name.as_str()))
    };
    let url = discord::invite_url(&id);
    println!("{url}");
    if open {
        util::open_browser(&url)?;
        eprintln!("{} {}", style("✓").green().bold(), t!("invite.opened"));
    }
    if copy {
        util::copy_clipboard(&url)?;
        eprintln!("{} {}", style("✓").green().bold(), t!("invite.copied"));
    }
    Ok(())
}
