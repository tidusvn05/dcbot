use anyhow::{bail, Result};
use console::style;
use rust_i18n::t;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::cmds::new::RUN_SH;
use crate::resolve::resolve;
use crate::tmux;

pub fn start(name: &str, respawn: bool) -> Result<()> {
    if !tmux::installed() {
        bail!(t!("lifecycle.tmux_missing"));
    }
    let bot = resolve(Some(name))?;
    let run_sh = bot.dir.join("run.sh");
    if !run_sh.exists() {
        // Self-heal: manifest exists but run.sh was deleted.
        fs::write(&run_sh, RUN_SH)?;
        fs::set_permissions(&run_sh, fs::Permissions::from_mode(0o755))?;
    }
    let session = tmux::session_name(&bot.name);
    if tmux::exists(&session) {
        bail!(t!("lifecycle.already_running", name = bot.name.as_str()));
    }
    tmux::start(&session, &bot.dir, respawn)?;
    println!(
        "{} {}",
        style("✓").green().bold(),
        t!(
            "lifecycle.started",
            name = bot.name.as_str(),
            session = session.as_str()
        )
    );
    println!(
        "  {}",
        t!("lifecycle.attach_hint", name = bot.name.as_str())
    );
    Ok(())
}

pub fn stop(name: &str) -> Result<()> {
    let bot = resolve(Some(name))?;
    let session = tmux::session_name(&bot.name);
    tmux::stop(&session)?;
    println!(
        "{} {}",
        style("✓").green().bold(),
        t!("lifecycle.stopped", name = bot.name.as_str())
    );
    Ok(())
}

pub fn restart(name: &str, respawn: bool) -> Result<()> {
    let bot = resolve(Some(name))?;
    let session = tmux::session_name(&bot.name);
    if tmux::exists(&session) {
        tmux::stop(&session)?;
    }
    start(&bot.name, respawn)
}

pub fn attach(name: &str) -> Result<()> {
    let bot = resolve(Some(name))?;
    let session = tmux::session_name(&bot.name);
    if !tmux::exists(&session) {
        bail!(t!("lifecycle.not_running", name = bot.name.as_str()));
    }
    let code = tmux::attach(&session)?;
    if code != 0 {
        bail!("tmux attach exited with {code}");
    }
    Ok(())
}

pub fn logs(name: &str, lines: u32, follow: bool) -> Result<()> {
    let bot = resolve(Some(name))?;
    let session = tmux::session_name(&bot.name);
    if !tmux::exists(&session) {
        bail!(t!("lifecycle.not_running", name = bot.name.as_str()));
    }
    let mut printed = 0usize;
    loop {
        let body = tmux::capture(&session, lines)?;
        let all: Vec<&str> = body.lines().collect();
        let fresh = if follow && all.len() > printed {
            &all[printed..]
        } else if !follow {
            &all[..]
        } else {
            &[][..]
        };
        for l in fresh {
            println!("{l}");
        }
        printed = all.len();
        if !follow {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}
