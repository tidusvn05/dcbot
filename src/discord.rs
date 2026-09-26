use anyhow::{bail, Context, Result};
use serde::Deserialize;

const API: &str = "https://discord.com/api/v10";

/// Dev/test hook: DCBOT_API_BASE redirects API calls to a local stub.
/// Unset → the real api.discord.com (the only host the token ever sees).
fn api_base() -> String {
    std::env::var("DCBOT_API_BASE").unwrap_or_else(|_| API.to_string())
}

/// Invite-URL permission bits: ADD_REACTIONS | VIEW_CHANNEL | SEND_MESSAGES
/// | ATTACH_FILES | READ_MESSAGE_HISTORY | SEND_MESSAGES_IN_THREADS.
pub const INVITE_PERMISSIONS: u64 =
    (1 << 6) | (1 << 10) | (1 << 11) | (1 << 15) | (1 << 16) | (1 << 38);

/// application.flags bits for the Message Content gateway intent. The
/// plain bit (1<<18) covers bots verified for 100+ guilds; the LIMITED
/// bit (1<<19) is what Discord sets for smaller bots when the toggle
/// is on — check either.
pub const FLAGS_MESSAGE_CONTENT: u64 = (1 << 18) | (1 << 19);

/// Deterministic OAuth2 invite URL — replaces the portal's URL Generator.
pub fn invite_url(client_id: &str) -> String {
    format!(
        "https://discord.com/oauth2/authorize?client_id={client_id}&scope=bot&permissions={INVITE_PERMISSIONS}"
    )
}

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
    let res = ureq::get(format!("{}/users/@me", api_base()))
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

/// The bot's application — `id` is the OAuth2 client_id; `flags` carries
/// the gateway-intent bits (absent on partial responses).
#[derive(Debug, Deserialize)]
pub struct AppInfo {
    pub id: String,
    pub flags: Option<u64>,
}

impl AppInfo {
    /// None = flags field missing, can't tell; Some(false) = intent OFF.
    pub fn message_content_intent(&self) -> Option<bool> {
        self.flags.map(|f| f & FLAGS_MESSAGE_CONTENT != 0)
    }
}

/// POST helper — same host rule as the GETs (api.discord.com, or the
/// DCBOT_API_BASE stub).
fn post(token: &str, path: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
    let res = ureq::post(format!("{}{path}", api_base()))
        .header("Authorization", &format!("Bot {token}"))
        .header("Content-Type", "application/json")
        .send(body.to_string());
    match res {
        Ok(mut r) => {
            let raw = r
                .body_mut()
                .read_to_string()
                .context("reading Discord response")?;
            serde_json::from_str(&raw).context("unexpected response from Discord")
        }
        Err(ureq::Error::StatusCode(401)) => bail!("invalid bot token (401 Unauthorized)"),
        Err(ureq::Error::StatusCode(c)) => bail!("Discord API error: HTTP {c}"),
        Err(e) => bail!("could not reach Discord API: {e}"),
    }
}

/// POST /users/@me/channels — open (or fetch) the DM channel with a user.
/// Returns the channel id for send_message.
pub fn open_dm(token: &str, user_id: &str) -> Result<String> {
    let v = post(
        token,
        "/users/@me/channels",
        &serde_json::json!({"recipient_id": user_id}),
    )?;
    v.get("id")
        .and_then(|i| i.as_str())
        .map(|s| s.to_string())
        .context("Discord response missing channel id")
}

/// POST /channels/{id}/messages.
pub fn send_message(token: &str, channel_id: &str, content: &str) -> Result<()> {
    post(
        token,
        &format!("/channels/{channel_id}/messages"),
        &serde_json::json!({"content": content}),
    )?;
    Ok(())
}

/// GET /applications/@me with the bot token — the client_id for the invite
/// URL plus the intent flags.
pub fn fetch_application(token: &str) -> Result<AppInfo> {
    let res = ureq::get(format!("{}/applications/@me", api_base()))
        .header("Authorization", &format!("Bot {token}"))
        .call();
    match res {
        Ok(mut r) => {
            let body = r
                .body_mut()
                .read_to_string()
                .context("reading Discord response")?;
            serde_json::from_str::<AppInfo>(&body).context("unexpected response from Discord")
        }
        Err(ureq::Error::StatusCode(401)) => bail!("invalid bot token (401 Unauthorized)"),
        Err(ureq::Error::StatusCode(c)) => bail!("Discord API error: HTTP {c}"),
        Err(e) => bail!("could not reach Discord API: {e}"),
    }
}
