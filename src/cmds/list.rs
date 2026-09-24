use anyhow::Result;
use comfy_table::{presets::UTF8_FULL, Cell, Color, Table};
use console::style;
use rust_i18n::t;
use std::collections::BTreeMap;

use crate::discord;
use crate::registry::Registry;
use crate::resolve::resolve;
use crate::state;
use crate::tmux;

pub fn list() -> Result<()> {
    let reg = Registry::load();
    let sessions = tmux::sessions();
    let live: std::collections::BTreeSet<String> = sessions.iter().cloned().collect();

    let mut table = Table::new();
    table.load_preset(UTF8_FULL).set_header(vec![
        t!("list.col_name").to_string(),
        t!("list.col_status").to_string(),
        t!("list.col_bot").to_string(),
        t!("list.col_dir").to_string(),
    ]);

    let mut id_seen: BTreeMap<&str, &str> = BTreeMap::new(); // bot_user_id → name
    let mut warnings: Vec<String> = Vec::new();

    for (name, e) in &reg.bots {
        let session = tmux::session_name(name);
        let dir_exists = e.dir.is_dir();
        let status = if !dir_exists {
            warnings.push(t!("list.warn_missing_dir", name = name.as_str()).to_string());
            Cell::new(t!("list.st_missing")).fg(Color::Red)
        } else if live.contains(&session) {
            Cell::new(t!("list.st_running")).fg(Color::Green)
        } else {
            Cell::new(t!("list.st_stopped")).fg(Color::DarkGrey)
        };
        if let Some(prev) = id_seen.insert(e.bot_user_id.as_str(), name.as_str()) {
            if prev != name.as_str() {
                warnings.push(t!("list.warn_dup_token", a = prev, b = name.as_str()).to_string());
            }
        }
        table.add_row(vec![
            Cell::new(name),
            status,
            Cell::new(&e.bot_tag),
            Cell::new(e.dir.display().to_string()),
        ]);
    }
    println!("{table}");

    // Orphan tmux sessions (dcbot-* with no registry entry).
    for s in &sessions {
        let bot = s.trim_start_matches("dcbot-");
        if !reg.bots.contains_key(bot) {
            warnings.push(t!("list.warn_orphan", session = s.as_str()).to_string());
        }
    }
    if reg.bots.is_empty() {
        println!("{}", t!("list.empty"));
    }
    for w in warnings {
        eprintln!("{} {w}", style(t!("common.warn")).yellow().bold());
    }
    Ok(())
}

pub fn status(name: Option<&str>) -> Result<()> {
    let bot = resolve(name)?;
    println!("{} {}", style(t!("status.bot")).bold(), bot.name);
    println!("  {} {}", t!("status.dir"), bot.dir.display());
    if let Some(m) = &bot.manifest {
        println!(
            "  {} {} ({})",
            t!("status.identity"),
            m.bot_tag,
            m.bot_user_id
        );
        println!("  {} {}", t!("status.created"), m.created_at);
    } else {
        println!("  {}", style(t!("status.no_manifest")).yellow());
    }

    // Token / live identity
    match state::read_token(&bot.state_dir) {
        Some(tok) => match discord::fetch_me(&tok) {
            Ok(u) => println!("  {} {} ({})", t!("status.token_ok"), u.tag(), u.id),
            Err(e) => println!("  {} {e}", style(t!("status.token_bad")).red()),
        },
        None => println!("  {}", style(t!("status.token_missing")).red()),
    }

    // Access summary
    match state::load(&bot.state_dir) {
        Ok(a) => {
            println!(
                "  {} {} — {}: {}, {}: {}, {}: {}",
                t!("status.access"),
                a.dm_policy,
                t!("status.allow"),
                a.allow_from.len(),
                t!("status.pending"),
                a.pending.len(),
                t!("status.groups"),
                a.groups.len()
            );
        }
        Err(e) => println!("  {} {e}", style(t!("status.token_bad")).red()),
    }

    // Runtime
    let session = tmux::session_name(&bot.name);
    if tmux::exists(&session) {
        println!("  {} {}", t!("status.session"), session);
        if let Ok(pane) = tmux::capture(&session, 200) {
            if let Some(line) = pane
                .lines()
                .rev()
                .find(|l| l.contains("gateway connected as"))
            {
                println!("  {} {}", t!("status.gateway"), line.trim());
            }
        }
    } else {
        println!("  {} {}", t!("status.session"), t!("list.st_stopped"));
    }
    Ok(())
}
