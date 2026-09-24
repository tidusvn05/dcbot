use anyhow::Result;
use console::style;
use rust_i18n::t;
use std::fs;
use std::os::unix::fs::PermissionsExt;

use crate::discord;
use crate::registry::Registry;
use crate::resolve::resolve;
use crate::state;
use crate::tmux;

enum Level {
    Pass,
    Warn,
    Fail,
    Info,
}

struct Check {
    level: Level,
    label: String,
    detail: String,
}

fn report(c: &Check) {
    let tag = match c.level {
        Level::Pass => style("PASS").green().bold(),
        Level::Warn => style("WARN").yellow().bold(),
        Level::Fail => style("FAIL").red().bold(),
        Level::Info => style("INFO").cyan().bold(),
    };
    if c.detail.is_empty() {
        println!("[{tag}] {}", c.label);
    } else {
        println!("[{tag}] {} — {}", c.label, c.detail);
    }
}

/// Health checks for one deployment. `target` = registry name or path; None = cwd.
pub fn run(target: Option<&str>) -> Result<i32> {
    let bot = resolve(target)?;
    let mut checks: Vec<Check> = Vec::new();
    println!(
        "{} {} ({})\n",
        style(t!("doctor.title")).bold(),
        bot.name,
        bot.dir.display()
    );

    // 1. state dir
    let state_ok = bot.state_dir.is_dir();
    checks.push(Check {
        level: if state_ok { Level::Pass } else { Level::Fail },
        label: t!("doctor.state_dir").to_string(),
        detail: bot.state_dir.display().to_string(),
    });

    // 2. .env + perms
    let env_file = bot.state_dir.join(".env");
    match fs::metadata(&env_file) {
        Ok(md) => {
            checks.push(Check {
                level: Level::Pass,
                label: t!("doctor.env_exists").to_string(),
                detail: String::new(),
            });
            let mode = md.permissions().mode() & 0o777;
            checks.push(Check {
                level: if mode == 0o600 {
                    Level::Pass
                } else {
                    Level::Warn
                },
                label: t!("doctor.env_perms").to_string(),
                detail: format!("{:o}", mode),
            });
        }
        Err(_) => checks.push(Check {
            level: Level::Fail,
            label: t!("doctor.env_exists").to_string(),
            detail: t!("doctor.env_missing").to_string(),
        }),
    }

    // 3. token live
    let mut bot_user_id: Option<String> = None;
    match state::read_token(&bot.state_dir) {
        Some(tok) => match discord::fetch_me(&tok) {
            Ok(u) => {
                bot_user_id = Some(u.id.clone());
                checks.push(Check {
                    level: Level::Pass,
                    label: t!("doctor.token_valid").to_string(),
                    detail: format!("{} ({})", u.tag(), u.id),
                });
            }
            Err(e) => checks.push(Check {
                level: Level::Fail,
                label: t!("doctor.token_valid").to_string(),
                detail: e.to_string(),
            }),
        },
        None => checks.push(Check {
            level: Level::Fail,
            label: t!("doctor.token_valid").to_string(),
            detail: t!("doctor.env_missing").to_string(),
        }),
    }

    // 3b. Message Content Intent (skipped when Discord won't say)
    if let Some(tok) = state::read_token(&bot.state_dir) {
        if let Ok(app) = discord::fetch_application(&tok) {
            if let Some(on) = app.message_content_intent() {
                checks.push(Check {
                    level: if on { Level::Pass } else { Level::Warn },
                    label: t!("doctor.intent").to_string(),
                    detail: if on {
                        String::new()
                    } else {
                        t!("doctor.intent_off").to_string()
                    },
                });
            }
        }
    }

    // 4. manifest ↔ token identity
    match (&bot.manifest, &bot_user_id) {
        (Some(m), Some(id)) => checks.push(Check {
            level: if &m.bot_user_id == id {
                Level::Pass
            } else {
                Level::Warn
            },
            label: t!("doctor.manifest_match").to_string(),
            detail: format!("manifest={} live={}", m.bot_user_id, id),
        }),
        (None, _) => checks.push(Check {
            level: Level::Warn,
            label: t!("doctor.manifest_match").to_string(),
            detail: t!("status.no_manifest").to_string(),
        }),
        _ => {}
    }

    // 5. access.json
    match state::load(&bot.state_dir) {
        Ok(a) => {
            checks.push(Check {
                level: Level::Pass,
                label: t!("doctor.access_parses").to_string(),
                detail: format!(
                    "policy={} allow={} pending={} groups={}",
                    a.dm_policy,
                    a.allow_from.len(),
                    a.pending.len(),
                    a.groups.len()
                ),
            });
            if a.dm_policy == "pairing" {
                checks.push(Check {
                    level: Level::Warn,
                    label: t!("doctor.policy_pairing").to_string(),
                    detail: t!("access.pairing_warn").to_string(),
                });
            }
        }
        Err(e) => checks.push(Check {
            level: Level::Fail,
            label: t!("doctor.access_parses").to_string(),
            detail: e.to_string(),
        }),
    }

    // 6. tools on PATH
    for tool in ["claude", "bun", "tmux"] {
        checks.push(Check {
            level: if which::which(tool).is_ok() {
                Level::Pass
            } else {
                Level::Fail
            },
            label: format!("{} `{tool}`", t!("doctor.tool")),
            detail: String::new(),
        });
    }

    // 7. plugin installed (best-effort scan under ~/.claude)
    let plugin_found = dirs::home_dir()
        .map(|h| h.join(".claude"))
        .and_then(|d| find_discord_plugin(&d, 3))
        .is_some();
    checks.push(Check {
        level: if plugin_found {
            Level::Pass
        } else {
            Level::Warn
        },
        label: t!("doctor.plugin").to_string(),
        detail: if plugin_found {
            String::new()
        } else {
            t!("doctor.plugin_hint").to_string()
        },
    });

    // 8. run.sh
    let run_sh = bot.dir.join("run.sh");
    match fs::read_to_string(&run_sh) {
        Ok(body) => {
            let good = body.contains("DISCORD_STATE_DIR") && body.contains("--channels");
            checks.push(Check {
                level: if good { Level::Pass } else { Level::Warn },
                label: t!("doctor.run_sh").to_string(),
                detail: if good {
                    String::new()
                } else {
                    t!("doctor.run_sh_bad").to_string()
                },
            });
        }
        Err(_) => checks.push(Check {
            level: Level::Warn,
            label: t!("doctor.run_sh").to_string(),
            detail: t!("doctor.run_sh_missing").to_string(),
        }),
    }

    // 9. duplicate bot identity across registry
    if let Some(id) = &bot_user_id {
        let reg = Registry::load();
        let dup: Vec<&String> = reg
            .bots
            .iter()
            .filter(|(n, e)| &e.bot_user_id == id && *n != &bot.name)
            .map(|(n, _)| n)
            .collect();
        if !dup.is_empty() {
            checks.push(Check {
                level: Level::Warn,
                label: t!("doctor.dup_token").to_string(),
                detail: dup
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>()
                    .join(", "),
            });
        }
    }

    // 10. runtime state
    let session = tmux::session_name(&bot.name);
    checks.push(Check {
        level: Level::Info,
        label: t!("doctor.session").to_string(),
        detail: format!(
            "{session}: {}",
            if tmux::exists(&session) {
                t!("list.st_running")
            } else {
                t!("list.st_stopped")
            }
        ),
    });

    let mut exit = 0;
    for c in &checks {
        report(c);
        if matches!(c.level, Level::Fail) {
            exit = 1;
        }
    }
    Ok(exit)
}

/// Look for the discord plugin under ~/.claude (plugins/, marketplaces/).
fn find_discord_plugin(dir: &std::path::Path, depth: u8) -> Option<()> {
    if depth == 0 {
        return None;
    }
    let rd = fs::read_dir(dir).ok()?;
    for e in rd.flatten() {
        let p = e.path();
        let name = e.file_name().to_string_lossy().to_lowercase();
        if name == "discord" {
            return Some(());
        }
        if p.is_dir() && !name.starts_with('.') && find_discord_plugin(&p, depth - 1).is_some() {
            return Some(());
        }
    }
    None
}
