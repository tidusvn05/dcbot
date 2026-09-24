use anyhow::{bail, Result};
use rust_i18n::t;

use crate::discord;
use crate::registry::Registry;
use crate::resolve::resolve;

/// Print the OAuth2 invite URL — client_id from bot.toml, falling back to
/// the bot user id (identical on standard Discord apps) in the registry.
pub fn run(name: Option<&str>) -> Result<()> {
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
    match client_id {
        Some(id) => {
            println!("{}", discord::invite_url(&id));
            Ok(())
        }
        None => bail!(t!("invite.no_id", name = bot.name.as_str())),
    }
}
