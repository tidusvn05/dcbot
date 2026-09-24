# dcbot

[![CI](https://github.com/your-org/dcbot/actions/workflows/ci.yml/badge.svg)](https://github.com/your-org/dcbot/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/your-org/dcbot)](https://github.com/your-org/dcbot/releases)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

CLI companion for [`discord@claude-plugins-official`](https://github.com/anthropics/claude-plugins-official/tree/main/external_plugins/discord) — onboard and operate **multiple Discord-channel bots for Claude Code** on one machine, each with its own `.discord-state` instead of the shared global `~/.claude/channels/discord`.

Why: the official plugin keeps state in one global directory and its `/discord:access` skill hardcodes that path. dcbot gives every bot its own deployment dir, manages the tmux session that runs `claude --channels …`, and performs access-control operations (approve/allow/policy/groups) directly on each bot's local `access.json`.

**Languages:** English (default) · [Tiếng Việt](docs/README.vi.md) · [日本語](docs/README.ja.md) — switch with `dcbot config set lang <en|vi|ja>` or `--lang`.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/your-org/dcbot/main/install.sh | bash
```

Pin a version: `curl -fsSL …/install.sh | DCBOT_VERSION=v0.1.0 bash`

Or from source: `cargo install --path .` (Rust stable).

**Requirements:** [Claude Code](https://docs.anthropic.com/en/docs/claude-code) with the discord plugin (`/plugin install discord@claude-plugins-official`), `tmux`, `bun`. Linux & macOS.

## Quick start

```bash
dcbot new helper        # wizard: portal guide → token → validate → seed allowlist
dcbot attach helper     # jump into the tmux session running claude
dcbot list              # all bots on this machine
dcbot doctor            # health-check the deployment in the current dir
```

Inside the tmux session, claude runs with `DISCORD_STATE_DIR=<dir>/.discord-state` — DM your bot and you're talking to that session.

## Commands

| Command | What it does |
| --- | --- |
| `dcbot new <name> [--dir p \| --here]` | Onboarding wizard — creates dir, `.discord-state/.env` (600), seeded `access.json`, `bot.toml`, `run.sh`, `.gitignore`; registers the bot |
| `dcbot start/stop/restart <name>` | tmux lifecycle (`--respawn` auto-restarts claude on exit) |
| `dcbot attach <name>` | `tmux attach -t dcbot-<name>` |
| `dcbot logs <name> [-f]` | Tail the session pane |
| `dcbot list` | Registry ⨝ tmux — running/stopped/missing, duplicate-token & orphan warnings |
| `dcbot status [name]` | Token live-check, gateway line, allowlist/pending counts |
| `dcbot doctor [target]` | `.env` perms, token validity, access.json, tools on PATH, plugin presence, run.sh, duplicate tokens |
| `dcbot approve <code>` | Approve pairing → `allowFrom` + writes `approved/<senderId>` marker |
| `dcbot deny / allow / remove / policy` | Manage `access.json` for the bot in cwd (or named) |
| `dcbot group add/rm <channelId>` | Guild-channel opt-in (`--no-mention`, `--allow ids`) |
| `dcbot set <key> <value>` | `ackReaction`, `replyToMode`, `textChunkLimit`, `chunkMode`, `mentionPatterns` |
| `dcbot register <dir> / forget / prune` | Registry management & drift cleanup |
| `dcbot config set lang <en\|vi\|ja>` | Persist UI language |
| `dcbot completions <shell>` | Shell completions |

Name arguments are optional inside a deployment dir — dcbot resolves the bot by walking up to the nearest `.discord-state`.

## Layout

```
<deployment>/                # anywhere on disk
  bot.toml                   # manifest (bot id/tag, created, channels flag)
  run.sh                     # exported DISCORD_STATE_DIR → exec claude --channels …
  .discord-state/
    .env                     # DISCORD_BOT_TOKEN (600)
    access.json              # dmPolicy / allowFrom / groups / pending / delivery config
    approved/  inbox/
  logs/

~/.config/dcbot/
  config.toml                # lang
  bots.toml                  # registry: name → dir index (manifest is source of truth)
```

## Access model (same semantics as the upstream plugin)

- `dmPolicy`: `pairing` (default — unknown sender gets a code), `allowlist` (drop silently), `disabled`.
- `approve` moves a pending `senderId` into `allowFrom` and drops the `approved/<senderId>` marker the server polls for.
- Guild channels are opt-in per **channel** snowflake; threads inherit the parent; `requireMention` defaults to true.
- `dcbot new` seeds `allowlist` with your snowflake when provided — the lockdown the plugin recommends.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
