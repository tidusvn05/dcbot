use anyhow::{bail, Context, Result};
use serde::Deserialize;

const API: &str = "https://discord.com/api/v10";

#[derive(Debug, Deserialize)]
pub struct BotUser {
    pub id: String,
    pub username: String,
    #[serde(default)]
    pub discriminator: String,
}

impl BotUser {
    /// "name" for new usernames (discriminator "0"), "name#1234" for legacy.
    pub fn tag(&self) -> String {
        if self.discriminator.is_empty() || self.discriminator == "0" {
            format!("@{}", self.username)
        } else {
            format!("{}#{}", self.username, self.discriminator)
        }
    }
}

/// GET /users/@me with the bot token — validates the token and returns
/// the bot identity. 401 = bad token.
pub fn fetch_me(token: &str) -> Result<BotUser> {
    let res = ureq::get(format!("{API}/users/@me"))
        .header("Authorization", &format!("Bot {token}"))
        .call();
    match res {
        Ok(mut r) => {
            let body = r
                .body_mut()
                .read_to_string()
                .context("reading Discord response")?;
            serde_json::from_str::<BotUser>(&body).context("unexpected response from Discord")
        }
        Err(ureq::Error::StatusCode(401)) => bail!("invalid bot token (401 Unauthorized)"),
        Err(ureq::Error::StatusCode(c)) => bail!("Discord API error: HTTP {c}"),
        Err(e) => bail!("could not reach Discord API: {e}"),
    }
}
