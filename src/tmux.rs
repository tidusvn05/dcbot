use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::{Command, Stdio};

/// Marker the tmux wrapper prints after run.sh exits, right before the
/// fallback shell. Its presence in scrollback = claude is dead, the pane
/// is just a shell.
pub const DEAD_MARKER: &str = "run.sh exited — shell below";
/// The channel server announces a successful Discord login on stderr —
/// the only true "bot can hear DMs" signal.
pub const GATEWAY_MARKER: &str = "gateway connected as";

/// What a session's pane is doing right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneState {
    /// No tmux session.
    Gone,
    /// Foreground process is `claude`.
    Claude,
    /// run.sh finished and the pane dropped to its fallback login shell —
    /// nothing is listening on Discord even though the session "exists".
    DeadShell,
    /// Anything else: still booting (`sh -c`, `run.sh`, the respawn
    /// loop's sleep) or a process we don't recognize.
    Other(String),
}

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

/// pane_current_command + pane_pid of the session's first pane.
fn pane_info(session: &str) -> Option<(String, i32)> {
    let out = Command::new("tmux")
        .args([
            "list-panes",
            "-t",
            session,
            "-F",
            "#{pane_current_command} #{pane_pid}",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let line = String::from_utf8_lossy(&out.stdout);
    let mut parts = line.split_whitespace();
    let cmd = parts.next()?.to_string();
    let pid = parts.next()?.parse().ok()?;
    Some((cmd, pid))
}

/// The pane pid's argv — after `exec bash -l` it reads `bash -l`; during
/// the run.sh chain it's `sh -c '<cmd>'` or `bash ./run.sh`.
fn pane_argv(pid: i32) -> Vec<String> {
    std::fs::read(format!("/proc/{pid}/cmdline"))
        .map(|raw| {
            raw.split(|b| *b == 0)
                .filter(|s| !s.is_empty())
                .map(|s| String::from_utf8_lossy(s).to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// Every descendant pid of `pid` (pgrep -P, breadth-first).
fn descendants(pid: i32) -> Vec<i32> {
    let mut all = Vec::new();
    let mut stack = vec![pid];
    while let Some(p) = stack.pop() {
        if let Ok(out) = Command::new("pgrep").args(["-P", &p.to_string()]).output() {
            for l in String::from_utf8_lossy(&out.stdout).lines() {
                if let Ok(c) = l.trim().parse::<i32>() {
                    all.push(c);
                    stack.push(c);
                }
            }
        }
    }
    all
}

fn proc_cmdline(pid: i32) -> String {
    std::fs::read(format!("/proc/{pid}/cmdline"))
        .map(|raw| {
            raw.split(|b| *b == 0)
                .filter(|s| !s.is_empty())
                .map(|s| String::from_utf8_lossy(s).to_string())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default()
}

fn proc_comm(pid: i32) -> String {
    std::fs::read_to_string(format!("/proc/{pid}/comm"))
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

/// Classify the session's pane. DeadShell requires positive evidence —
/// the fallback marker in scrollback, or the pane process being a login
/// shell — because mid-boot the foreground can also read as sh/bash.
pub fn pane_state(session: &str) -> PaneState {
    let Some((cmd, pid)) = pane_info(session) else {
        return PaneState::Gone;
    };
    // claude may be exec'd by the wrapper (pane comm = "bash" while a
    // claude child runs) — check the whole process tree, not just the
    // foreground command.
    if cmd == "claude"
        || descendants(pid).iter().any(|p| proc_comm(*p) == "claude")
    {
        return PaneState::Claude;
    }
    let argv = pane_argv(pid);
    let login_shell = argv
        .first()
        .map(|a| {
            a.starts_with('-') || argv.iter().skip(1).any(|a| a == "-l")
        })
        .unwrap_or(false);
    if login_shell {
        return PaneState::DeadShell;
    }
    if let Ok(body) = capture(session, 3000) {
        if body.contains(DEAD_MARKER) {
            return PaneState::DeadShell;
        }
    }
    PaneState::Other(cmd)
}

/// The "gateway connected as <tag>" line the channel server writes when
/// the Discord gateway logs in — searched over the whole scrollback.
/// NOTE: server.ts writes it to stderr, which claude captures into a
/// socket — it does NOT reliably reach the pane. Prefer channel_status.
fn gateway_line(session: &str) -> Option<String> {
    let out = Command::new("tmux")
        .args(["capture-pane", "-p", "-t", session, "-S", "-"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .rev()
        .find(|l| l.contains(GATEWAY_MARKER))
        .map(|l| l.trim().to_string())
}

/// Proof the Discord channel server is alive: a `server.ts` descendant
/// of the pane. server.ts exits on gateway login failure, so its
/// presence means the token authenticated and the gateway is connected
/// (or connecting — the process dies fast when it can't).
fn channel_server_pid(session: &str) -> Option<i32> {
    let (_, pid) = pane_info(session)?;
    descendants(pid)
        .into_iter()
        .find(|p| proc_cmdline(*p).contains("server.ts"))
}

/// Best available "the bot can hear DMs" evidence for this session —
/// the gateway scrollback line when it's visible, else the live
/// channel-server process. None = nothing is listening.
pub fn channel_status(session: &str) -> Option<String> {
    if let Some(line) = gateway_line(session) {
        return Some(line);
    }
    channel_server_pid(session).map(|pid| format!("channel server running (pid {pid})"))
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
        "{arg}; echo; echo '[dcbot] {DEAD_MARKER}'; exec ${{SHELL:-bash}} -l"
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
        kill(session)?;
    }
    Ok(())
}

/// Hard kill — for panes that are already dead shells, where C-c would
/// just ring the bell.
pub fn kill(session: &str) -> Result<()> {
    let status = Command::new("tmux")
        .args(["kill-session", "-t", session])
        .status()
        .context("tmux kill-session failed")?;
    if !status.success() {
        bail!("tmux kill-session '{session}' exited {status}");
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
