use anyhow::{bail, Context, Result};
use std::io::Write;
use std::process::{Command, Stdio};

/// Launch the system browser — stdio silenced so the TTY stays clean even
/// if the opener is chatty. Waits for the handoff so headless failures
/// (xdg-open with no browser) surface instead of lying "opened".
pub fn open_browser(url: &str) -> Result<()> {
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let status = Command::new(opener)
        .arg(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .with_context(|| format!("{opener} not found"))?;
    if !status.success() {
        bail!("{opener} exited with {status}");
    }
    Ok(())
}

/// Best-effort clipboard write across macOS / Wayland / X11.
pub fn copy_clipboard(text: &str) -> Result<()> {
    let tools: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("pbcopy", &[])]
    } else {
        &[
            ("wl-copy", &[]),
            ("xclip", &["-selection", "clipboard"]),
            ("xsel", &["--clipboard", "--input"]),
        ]
    };
    for (cmd, args) in tools {
        if which::which(cmd).is_err() {
            continue;
        }
        let mut child = Command::new(cmd)
            .args(*args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("spawning {cmd}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes())?;
        }
        child.wait().with_context(|| format!("{cmd} failed"))?;
        return Ok(());
    }
    bail!("no clipboard tool found (tried pbcopy, wl-copy, xclip, xsel)")
}
