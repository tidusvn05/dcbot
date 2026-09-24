# dcbot — agent usage contract

dcbot manages Discord-channel bots for Claude Code on this machine. Every
bot lives in its own deployment dir containing `.discord-state/` — never
touch the global `~/.claude/channels/discord`.

All commands are non-interactive-safe when flags are given (no prompts).

## Commands

```
dcbot new <name> --yes [--dir <path>|--here]
    Onboard a bot. Token from $DCBOT_BOT_TOKEN or --token <t>;
    --owner <snowflake> seeds the allowlist (recommended; empty leaves
    pairing mode), --start launches the tmux session.
dcbot register <dir>     adopt an existing deployment (has bot.toml or
                         .discord-state); generates run.sh if missing
dcbot list / status [name] / doctor [name|dir]
dcbot invite [name]     re-print the OAuth2 invite URL (add the bot
                        to another server)
dcbot start|stop|restart <name> [--respawn] / attach <name> / logs <name> [-f]
dcbot approve|deny <pairing-code>  codes live in .discord-state/access.json
dcbot allow|remove <snowflake> / policy <pairing|allowlist|disabled>
dcbot group add <channelId> [--no-mention] [--allow id1,id2] / group rm <channelId>
dcbot set <key> <value>  ackReaction, replyToMode, textChunkLimit,
                         chunkMode, mentionPatterns
dcbot forget <name> / prune    registry cleanup (never deletes dirs)
```

Inside a deployment dir, `<name>` may be omitted — resolved via the
nearest `.discord-state` upward.

## Rules

- Never launch `claude --channels` outside `dcbot start` — without
  `DISCORD_STATE_DIR` the channel server targets the global dir.
- One token = one process. Two processes on the same token deliver
  every DM twice.
- Prereqs: `claude` + discord plugin (`/plugin install
  discord@claude-plugins-official`), `bun`, `tmux` on PATH.

## Token handling

- dcbot sends the token to exactly one place: `api.discord.com`
  (identity + application metadata) — then writes it to
  `.discord-state/.env` mode `0600`.
  It never touches any other network endpoint. (Dev/testing only:
  `DCBOT_API_BASE` can redirect these calls to a local stub.)
- A token pasted in chat transits the model provider once. If the user
  prefers not to, have them run this in their own terminal:

      dcbot new <name> --yes

  It prompts for the token with hidden input — nothing enters chat or
  argv; the token lands straight in `.discord-state/.env` (0600).
- Never echo, log, commit, or exfiltrate the token. `--token` in argv is
  visible to `ps` — prefer the prompted flow above on shared machines.

## User has no token yet → print these steps and wait

Discord Developer Portal → New Application → Bot → Reset Token →
enable Message Content Intent → then `dcbot new <name> --yes --token
<t> --owner <snowflake> --start` — it prints the OAuth2 invite URL;
open it and add the bot to a shared server. `dcbot invite <name>`
re-prints the URL later.

## Migrating a global bot

```
mv ~/.claude/channels/discord <dir>/.discord-state
dcbot register <dir> && cd <dir> && dcbot doctor && dcbot start <name>
```

Stop every claude session using the global channel first; delete the
old global dir after the migrated bot is verified healthy.
