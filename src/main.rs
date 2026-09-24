mod claude;
mod cmds;
mod config;
mod discord;
mod manifest;
mod registry;
mod resolve;
mod state;
mod tmux;
mod util;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use std::path::PathBuf;

rust_i18n::i18n!("locales", fallback = "en");

#[derive(Parser)]
#[command(
    name = "dcbot",
    version,
    about = "Onboard and manage Discord-channel bots for Claude Code",
    long_about = None
)]
struct Cli {
    /// Language override: en, vi, ja
    #[arg(long, global = true, value_parser = ["en", "vi", "ja"])]
    lang: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new bot deployment (interactive wizard, or flags for agents)
    New {
        /// Bot name (registry key + tmux session suffix)
        name: String,
        /// Deployment directory (default: ./<name>)
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Deploy in the current directory
        #[arg(long)]
        here: bool,
        /// Discord bot token (non-interactive; or set DCBOT_BOT_TOKEN)
        #[arg(long)]
        token: Option<String>,
        /// Your Discord user snowflake — seeds the allowlist
        #[arg(long)]
        owner: Option<String>,
        /// Non-interactive: no prompts (requires --token or DCBOT_BOT_TOKEN)
        #[arg(long)]
        yes: bool,
        /// Start the tmux session right after creating
        #[arg(long)]
        start: bool,
        /// Pairing mode only: wait 60s for your DM and auto-approve the code
        #[arg(long)]
        pair: bool,
    },
    /// Adopt an existing deployment dir into the registry
    Register { dir: PathBuf },
    /// Remove a bot from the registry (deployment dir untouched)
    Forget { name: String },
    /// Drop registry entries whose dirs no longer exist
    Prune,
    /// List registered bots and runtime state
    List,
    /// Detailed status of one bot (default: bot in current dir)
    Status { name: Option<String> },
    /// Print the bot's OAuth2 invite URL (add it to another server)
    Invite {
        name: Option<String>,
        /// Also open the URL in the system browser
        #[arg(long)]
        open: bool,
        /// Also copy the URL to the clipboard
        #[arg(long)]
        copy: bool,
    },
    /// Start the bot's claude session in tmux
    Start {
        name: String,
        /// Restart claude automatically if it exits
        #[arg(long)]
        respawn: bool,
    },
    /// Stop the bot's tmux session
    Stop { name: String },
    /// Restart the bot's tmux session
    Restart {
        name: String,
        #[arg(long)]
        respawn: bool,
    },
    /// Attach to the bot's tmux session
    Attach { name: String },
    /// Show recent session output
    Logs {
        name: String,
        #[arg(short, long, default_value = "200")]
        lines: u32,
        #[arg(short, long)]
        follow: bool,
    },
    /// Health-check a deployment (default: current dir)
    Doctor { target: Option<String> },
    /// Approve a pending pairing code (no code = list pending; `dcbot pair` alias)
    #[command(visible_alias = "pair")]
    Approve {
        code: Option<String>,
        /// Wait for the next pairing DM and auto-approve it (default: 60s)
        #[arg(long, num_args = 0..=1, default_missing_value = "60", value_name = "secs")]
        wait: Option<u64>,
        name: Option<String>,
    },
    /// Discard a pending pairing code
    Deny { code: String, name: Option<String> },
    /// Add a user snowflake to the allowlist
    Allow { id: String, name: Option<String> },
    /// Remove a user snowflake from the allowlist
    Remove { id: String, name: Option<String> },
    /// Set DM policy: pairing | allowlist | disabled
    Policy { mode: String, name: Option<String> },
    /// Manage guild-channel opt-in
    Group {
        #[command(subcommand)]
        action: GroupAction,
    },
    /// Set a delivery config key (ackReaction, replyToMode, textChunkLimit, chunkMode, mentionPatterns)
    Set {
        key: String,
        value: String,
        name: Option<String>,
    },
    /// DM an allowlisted user via the Discord API (default: first allowFrom entry)
    Dm {
        /// Message text
        text: String,
        /// Bot name (default: resolve from cwd)
        name: Option<String>,
        /// Recipient user snowflake — must be in the allowlist
        #[arg(long)]
        to: Option<String>,
    },
    /// CLI configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Print shell completions
    Completions { shell: Shell },
    /// Print the agent usage contract (AGENTS.md)
    #[command(
        name = "agents.md",
        visible_alias = "agents-md",
        alias = "agents",
        alias = "agent.md",
        alias = "agent-md",
        alias = "agent"
    )]
    AgentsMd,
}

#[derive(Subcommand)]
enum GroupAction {
    /// Enable a guild channel (by channel snowflake)
    Add {
        channel_id: String,
        /// Respond to every message, not just mentions
        #[arg(long)]
        no_mention: bool,
        /// Comma-separated user snowflakes allowed to trigger
        #[arg(long)]
        allow: Option<String>,
        /// Bot name (default: resolve from cwd)
        name: Option<String>,
    },
    /// Disable a guild channel
    Rm {
        channel_id: String,
        name: Option<String>,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Set a config value
    Set { key: String, value: String },
    /// Show current config
    Get,
}

fn main() -> Result<()> {
    // Exit quietly on SIGPIPE (`dcbot list | head`) instead of panicking.
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    // Locale pass 1: env + config + system (before clap builds help text).
    rust_i18n::set_locale(&config::resolve_lang(None));

    let cli = Cli::parse();
    // Locale pass 2: explicit --lang flag wins.
    if let Some(l) = &cli.lang {
        rust_i18n::set_locale(l);
    }

    match cli.command {
        Commands::New {
            name,
            dir,
            here,
            token,
            owner,
            yes,
            start,
            pair,
        } => cmds::new::run(cmds::new::NewOpts {
            name,
            dir,
            here,
            token,
            owner,
            yes,
            start,
            pair,
        }),
        Commands::Register { dir } => cmds::registry_cmds::register(dir),
        Commands::Forget { name } => cmds::registry_cmds::forget(&name),
        Commands::Prune => cmds::registry_cmds::prune(),
        Commands::List => cmds::list::list(),
        Commands::Status { name } => cmds::list::status(name.as_deref()),
        Commands::Invite { name, open, copy } => cmds::invite::run(name.as_deref(), open, copy),
        Commands::Start { name, respawn } => cmds::lifecycle::start(&name, respawn),
        Commands::Stop { name } => cmds::lifecycle::stop(&name),
        Commands::Restart { name, respawn } => cmds::lifecycle::restart(&name, respawn),
        Commands::Attach { name } => cmds::lifecycle::attach(&name),
        Commands::Logs {
            name,
            lines,
            follow,
        } => cmds::lifecycle::logs(&name, lines, follow),
        Commands::Doctor { target } => {
            let code = cmds::doctor::run(target.as_deref())?;
            std::process::exit(code);
        }
        Commands::Approve { code, wait, name } => {
            cmds::access::approve(code.as_deref(), wait, name.as_deref())
        }
        Commands::Deny { code, name } => cmds::access::deny(&code, name.as_deref()),
        Commands::Allow { id, name } => cmds::access::allow(&id, name.as_deref()),
        Commands::Remove { id, name } => cmds::access::remove(&id, name.as_deref()),
        Commands::Policy { mode, name } => cmds::access::policy(&mode, name.as_deref()),
        Commands::Group { action } => match action {
            GroupAction::Add {
                channel_id,
                no_mention,
                allow,
                name,
            } => {
                cmds::access::group_add(&channel_id, no_mention, allow.as_deref(), name.as_deref())
            }
            GroupAction::Rm { channel_id, name } => {
                cmds::access::group_rm(&channel_id, name.as_deref())
            }
        },
        Commands::Set { key, value, name } => cmds::access::set(&key, &value, name.as_deref()),
        Commands::Dm { text, name, to } => cmds::dm::send(&text, to.as_deref(), name.as_deref()),
        Commands::Config { action } => match action {
            ConfigAction::Set { key, value } => {
                if key == "lang" {
                    cmds::config_cmd::set_lang(&value)
                } else {
                    anyhow::bail!("unknown config key '{key}' — supported: lang")
                }
            }
            ConfigAction::Get => cmds::config_cmd::get(),
        },
        Commands::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "dcbot", &mut std::io::stdout());
            Ok(())
        }
        Commands::AgentsMd => {
            print!("{}", include_str!("../AGENTS.md"));
            Ok(())
        }
    }
}
