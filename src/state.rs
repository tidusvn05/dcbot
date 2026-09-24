use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Mirror of the plugin server's access.json schema (camelCase on disk).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Access {
    pub dm_policy: String,
    #[serde(default)]
    pub allow_from: Vec<String>,
    #[serde(default)]
    pub groups: BTreeMap<String, GroupPolicy>,
    #[serde(default)]
    pub pending: BTreeMap<String, Pending>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mention_patterns: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ack_reaction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reply_to_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_chunk_limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chunk_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupPolicy {
    #[serde(default = "default_true")]
    pub require_mention: bool,
    #[serde(default)]
    pub allow_from: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pending {
    pub sender_id: String,
    pub chat_id: String,
    pub created_at: i64,
    pub expires_at: i64,
    #[serde(default = "default_one")]
    pub replies: u32,
}

fn default_true() -> bool {
    true
}
fn default_one() -> u32 {
    1
}

impl Default for Access {
    fn default() -> Self {
        Access {
            dm_policy: "pairing".to_string(),
            allow_from: vec![],
            groups: BTreeMap::new(),
            pending: BTreeMap::new(),
            mention_patterns: None,
            ack_reaction: None,
            reply_to_mode: None,
            text_chunk_limit: None,
            chunk_mode: None,
        }
    }
}

pub const VALID_POLICIES: &[&str] = &["pairing", "allowlist", "disabled"];
pub const VALID_SET_KEYS: &[&str] = &[
    "ackReaction",
    "replyToMode",
    "textChunkLimit",
    "chunkMode",
    "mentionPatterns",
];

pub fn access_file(state_dir: &Path) -> PathBuf {
    state_dir.join("access.json")
}

pub fn approved_dir(state_dir: &Path) -> PathBuf {
    state_dir.join("approved")
}

/// Load access.json. Missing file = default (pairing, empty lists),
/// same semantics as the channel server.
pub fn load(state_dir: &Path) -> Result<Access> {
    let file = access_file(state_dir);
    let raw = match fs::read_to_string(&file) {
        Ok(r) => r,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Access::default()),
        Err(e) => return Err(e).with_context(|| file.display().to_string()),
    };
    let mut a: Access = serde_json::from_str(&raw)
        .with_context(|| format!("corrupt access.json: {}", file.display()))?;
    // Fill defaults the same way the server does for partial files.
    if !VALID_POLICIES.contains(&a.dm_policy.as_str()) {
        a.dm_policy = "pairing".to_string();
    }
    Ok(a)
}

/// Atomic write (tmp + rename), 0600 — matches the server's write style.
pub fn save(state_dir: &Path, a: &Access) -> Result<()> {
    fs::create_dir_all(state_dir)?;
    let file = access_file(state_dir);
    let tmp = file.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_string_pretty(a)? + "\n")?;
    fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
    fs::rename(&tmp, &file)?;
    Ok(())
}

/// Approve a pending pairing code: sender → allowFrom, drop the pending
/// entry, and write approved/<senderId> = chatId so the server sends the
/// "Paired!" confirmation on its next poll (~5s).
pub fn approve(state_dir: &Path, code: &str) -> Result<String> {
    let mut a = load(state_dir)?;
    let Some(p) = a.pending.remove(code) else {
        bail!("no pending pairing with code '{code}'");
    };
    if !a.allow_from.contains(&p.sender_id) {
        a.allow_from.push(p.sender_id.clone());
    }
    save(state_dir, &a)?;

    let dir = approved_dir(state_dir);
    fs::create_dir_all(&dir)?;
    fs::write(dir.join(&p.sender_id), &p.chat_id)?;
    Ok(p.sender_id)
}

pub fn deny(state_dir: &Path, code: &str) -> Result<String> {
    let mut a = load(state_dir)?;
    let Some(p) = a.pending.remove(code) else {
        bail!("no pending pairing with code '{code}'");
    };
    save(state_dir, &a)?;
    Ok(p.sender_id)
}

pub fn allow(state_dir: &Path, user_id: &str) -> Result<bool> {
    let mut a = load(state_dir)?;
    if a.allow_from.iter().any(|u| u == user_id) {
        return Ok(false);
    }
    a.allow_from.push(user_id.to_string());
    save(state_dir, &a)?;
    Ok(true)
}

pub fn remove(state_dir: &Path, user_id: &str) -> Result<bool> {
    let mut a = load(state_dir)?;
    let before = a.allow_from.len();
    a.allow_from.retain(|u| u != user_id);
    if a.allow_from.len() == before {
        return Ok(false);
    }
    save(state_dir, &a)?;
    Ok(true)
}

pub fn set_policy(state_dir: &Path, policy: &str) -> Result<()> {
    if !VALID_POLICIES.contains(&policy) {
        bail!(
            "invalid policy '{policy}' — expected one of: {}",
            VALID_POLICIES.join(", ")
        );
    }
    let mut a = load(state_dir)?;
    a.dm_policy = policy.to_string();
    save(state_dir, &a)
}

pub fn group_add(
    state_dir: &Path,
    channel_id: &str,
    require_mention: bool,
    allow_from: Vec<String>,
) -> Result<()> {
    let mut a = load(state_dir)?;
    a.groups.insert(
        channel_id.to_string(),
        GroupPolicy {
            require_mention,
            allow_from,
        },
    );
    save(state_dir, &a)
}

pub fn group_rm(state_dir: &Path, channel_id: &str) -> Result<bool> {
    let mut a = load(state_dir)?;
    if a.groups.remove(channel_id).is_none() {
        return Ok(false);
    }
    save(state_dir, &a)?;
    Ok(true)
}

/// Typed `set` for delivery/UX keys — same keys the /discord:access skill accepts.
pub fn set_key(state_dir: &Path, key: &str, value: &str) -> Result<()> {
    if !VALID_SET_KEYS.contains(&key) {
        bail!(
            "unknown key '{key}' — expected one of: {}",
            VALID_SET_KEYS.join(", ")
        );
    }
    let mut a = load(state_dir)?;
    match key {
        "ackReaction" => {
            a.ack_reaction = if value.is_empty() || value == "\"\"" {
                Some(String::new())
            } else {
                Some(value.to_string())
            };
        }
        "replyToMode" => {
            if !["off", "first", "all"].contains(&value) {
                bail!("replyToMode must be off|first|all");
            }
            a.reply_to_mode = Some(value.to_string());
        }
        "textChunkLimit" => {
            let n: u32 = value.parse().context("textChunkLimit must be a number")?;
            if n == 0 || n > 2000 {
                bail!("textChunkLimit must be 1..=2000 (Discord hard cap)");
            }
            a.text_chunk_limit = Some(n);
        }
        "chunkMode" => {
            if !["length", "newline"].contains(&value) {
                bail!("chunkMode must be length|newline");
            }
            a.chunk_mode = Some(value.to_string());
        }
        "mentionPatterns" => {
            let pats: Vec<String> = serde_json::from_str(value)
                .context("mentionPatterns must be a JSON array of strings")?;
            for p in &pats {
                regex::Regex::new(p).with_context(|| format!("invalid regex: {p}"))?;
            }
            a.mention_patterns = Some(pats);
        }
        _ => unreachable!(),
    }
    save(state_dir, &a)
}

/// Read DISCORD_BOT_TOKEN from the state dir's .env (shell env wins,
/// same precedence as the server).
pub fn read_token(state_dir: &Path) -> Option<String> {
    if let Ok(t) = std::env::var("DISCORD_BOT_TOKEN") {
        if !t.is_empty() {
            return Some(t);
        }
    }
    let raw = fs::read_to_string(state_dir.join(".env")).ok()?;
    for line in raw.lines() {
        if let Some(v) = line.strip_prefix("DISCORD_BOT_TOKEN=") {
            let v = v.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}
