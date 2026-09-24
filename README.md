# dcbot

[![CI](https://github.com/tidusvn05/dcbot/actions/workflows/ci.yml/badge.svg)](https://github.com/tidusvn05/dcbot/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/tidusvn05/dcbot)](https://github.com/tidusvn05/dcbot/releases)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)

CLI companion for [`discord@claude-plugins-official`](https://github.com/anthropics/claude-plugins-official/tree/main/external_plugins/discord) — onboard and operate **multiple Discord-channel bots for Claude Code** on one machine, each with its own `.discord-state` instead of the shared global `~/.claude/channels/discord`.

Why: the official plugin keeps state in one global directory and its `/discord:access` skill hardcodes that path. dcbot gives every bot its own deployment dir, manages the tmux session that runs `claude --channels …`, and performs access-control operations (approve/allow/policy/groups) directly on each bot's local `access.json`.

**Languages:** English (default) · [Tiếng Việt](docs/README.vi.md) · [日本語](docs/README.ja.md) — switch with `dcbot config set lang <en|vi|ja>` or `--lang`.

## Install

```bash
curl -fsSL https://raw.githubusercontent.com/tidusvn05/dcbot/main/install.sh | bash
```

**Requirements:** [Claude Code](https://docs.anthropic.com/en/docs/claude-code) with the discord plugin (`/plugin install discord@claude-plugins-official`), `tmux`, `bun`. Linux & macOS.

## Quick start

```bash
dcbot new business-bot        # wizard: portal guide → token → validate → seed allowlist
dcbot attach business-bot     # jump into the tmux session running claude
dcbot list              # all bots on this machine
dcbot doctor            # health-check the deployment in the current dir
```

Inside the tmux session, claude runs with `DISCORD_STATE_DIR=<dir>/.discord-state` — DM your bot and you're talking to that session.

## Guides

### Agent-first — hand it to your agent

dcbot ships its own usage contract — `dcbot agents.md` prints it (also
shipped as [`AGENTS.md`](AGENTS.md)). Point your agent at it, then ask in
natural language:

```text
read `dcbot agents.md`

which bots are running right now?
```

```text
read `dcbot agents.md`

create a new discord bot called business-bot, token is ...
```

> **Token in a prompt** transits your agent's model provider once. dcbot
> itself only sends it to `api.discord.com` (validation) and writes it to
> `.discord-state/.env` mode `0600` — nothing else on the public internet
> sees it. Rather keep it out of chat? Run `dcbot new business-bot
> --yes` yourself in a terminal — it asks only for the token (hidden
> input), then the agent can take over with `dcbot` commands.

```text
read `dcbot agents.md`

migrate my existing global discord bot into dcbot, name it legacy-bot
```

```text
read `dcbot agents.md`

approve pairing code a4f91c for business-bot
```

```text
read `dcbot agents.md`

doctor the deployment in this directory
```

Any language works — the ask after the first line is yours.

### Fresh setup — from `plugin install` to a running bot

Prereqs: `claude` (Claude Code), `tmux`, `bun`, a Discord account.

```bash
# 1. Install the channel plugin — inside any claude session:
/plugin install discord@claude-plugins-official

# 2. Install dcbot:
curl -fsSL https://raw.githubusercontent.com/tidusvn05/dcbot/main/install.sh | bash

# 3. Create a deployment. The wizard prints the Developer Portal steps
#    (New Application → Bot → Reset Token → enable Message Content Intent
#    → invite URL), validates your token live, seeds access.json with
#    your Discord snowflake (allowlist mode — no pairing needed), writes
#    run.sh, and registers the bot:
dcbot new business-bot                 # creates ./business-bot/
dcbot new business-bot --dir ~/bots/x  # or a specific path
dcbot new business-bot --here          # or deploy into the current dir

# 4. Run it:
dcbot start business-bot               # tmux session dcbot-business-bot
dcbot attach business-bot              # jump into the claude session
```

DM your bot — with your snowflake seeded it just works. If you left it empty (pairing mode), the bot replies with a code; approve it with `dcbot approve <code>` (run in the deployment dir or pass the bot name).

### Migrating an existing global bot

If your bot already runs with the global state dir (`~/.claude/channels/discord/`), move it under dcbot — the inner layout is identical, so it's a single `mv`:

```bash
# 1. Stop every claude session using the global channel. One token must
#    not run in two processes — every DM would be delivered twice.

# 2. Move the state dir into a deployment dir (.env, access.json,
#    approved/, inbox/ — allowlist, groups and pending pairings all kept):
mkdir -p ~/bots/mybot
mv ~/.claude/channels/discord ~/bots/mybot/.discord-state

# 3. Adopt it — validates the existing token, writes bot.toml + run.sh,
#    registers the bot:
dcbot register ~/bots/mybot
cd ~/bots/mybot && dcbot doctor   # verify the deployment is healthy

# 4. From now on, start the bot only through dcbot:
dcbot start mybot
```

After the move, do **not** launch `claude --channels …` manually anymore — without `DISCORD_STATE_DIR` the server falls back to the now-empty global dir and exits on a missing token. If you copied (rather than moved) the state dir, delete `~/.claude/channels/discord` once the migrated bot is verified healthy, so no stray session can resurrect the duplicate-token problem.

## Commands

| Command | What it does |
| --- | --- |
| `dcbot new <name> [--dir p \| --here \| --yes]` | Onboarding wizard — creates dir, `.discord-state/.env` (600), seeded `access.json`, `bot.toml`, `run.sh`, `.gitignore`; registers the bot. `--yes` runs non-interactively (`--token`/`$DCBOT_BOT_TOKEN`, `--owner`, `--start`) — agent-friendly |
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

## Other install options

Pin a release:

```bash
curl -fsSL https://raw.githubusercontent.com/tidusvn05/dcbot/main/install.sh | DCBOT_VERSION=v0.3.0 bash
```

From source (Rust stable):

```bash
cargo install --path .
```

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.
