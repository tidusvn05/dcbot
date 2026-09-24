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

## User has no token yet → print these steps and wait

Discord Developer Portal → New Application → Bot → Reset Token →
enable Message Content Intent → OAuth2 URL Generator (scope `bot`;
perms: View Channels, Send Messages, Send Messages in Threads, Read
Message History, Attach Files, Add Reactions) → invite the bot to a
shared server → then `dcbot new <name> --yes --token <t> --owner
<snowflake> --start`.

## Migrating a global bot

```
mv ~/.claude/channels/discord <dir>/.discord-state
dcbot register <dir> && cd <dir> && dcbot doctor && dcbot start <name>
```

Stop every claude session using the global channel first; delete the
old global dir after the migrated bot is verified healthy.
