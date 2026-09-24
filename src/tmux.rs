use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::{Command, Stdio};

pub fn installed() -> bool {
    which::which("tmux").is_ok()
}

pub fn session_name(bot: &str) -> String {
    format!("dcbot-{bot}")
}

pub fn exists(session: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", session])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// All live tmux sessions owned by dcbot (prefix `dcbot-`).
pub fn sessions() -> Vec<String> {
    let Ok(out) = Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_name}"])
        .output()
    else {
        return vec![];
    };
    if !out.status.success() {
        return vec![];
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|l| l.starts_with("dcbot-"))
        .map(|l| l.to_string())
        .collect()
}

/// Spawn the bot's run.sh in a detached tmux session. After run.sh exits
/// the pane drops to a shell instead of closing, so the exit state stays
/// visible on attach.
pub fn start(session: &str, dir: &Path, respawn: bool) -> Result<()> {
    let arg = if respawn {
        "./run.sh --respawn"
    } else {
        "./run.sh"
    };
    let cmd = format!(
        "{arg}; echo; echo '[dcbot] run.sh exited — shell below'; exec ${{SHELL:-bash}} -l"
    );
    let status = Command::new("tmux")
        .args(["new-session", "-d", "-s", session, "-c"])
        .arg(dir)
        .arg(&cmd)
        .status()
        .context("failed to spawn tmux")?;
    if !status.success() {
        bail!("tmux new-session failed (exit {status})");
    }
    Ok(())
}

/// Graceful stop: C-c first, then kill-session. Either way the pane's
/// death EOFs the MCP server's stdin, which shuts the gateway down
/// cleanly on its own.
pub fn stop(session: &str) -> Result<()> {
    if !exists(session) {
        bail!("session '{session}' is not running");
    }
    let _ = Command::new("tmux")
        .args(["send-keys", "-t", session, "C-c"])
        .status();
    std::thread::sleep(std::time::Duration::from_millis(1500));
    if exists(session) {
        let _ = Command::new("tmux")
            .args(["kill-session", "-t", session])
            .status();
    }
    Ok(())
}

/// Interactive attach — inherits stdio so the TUI works.
pub fn attach(session: &str) -> Result<i32> {
    let status = Command::new("tmux")
        .args(["attach-session", "-t", session])
        .status()
        .context("failed to run tmux attach")?;
    Ok(status.code().unwrap_or(1))
}

pub fn capture(session: &str, lines: u32) -> Result<String> {
    let out = Command::new("tmux")
        .args([
            "capture-pane",
            "-p",
            "-t",
            session,
            "-S",
            &format!("-{lines}"),
        ])
        .output()
        .context("failed to run tmux capture-pane")?;
    if !out.status.success() {
        bail!("tmux capture-pane failed for '{session}'");
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}
