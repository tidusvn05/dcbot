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
                        to another server); --open launches a browser,
                        --copy puts it on the clipboard
dcbot start|stop|restart <name> [--respawn] / attach <name> / logs <name> [-f]
dcbot approve|deny|pair <code>   codes live in .discord-state/access.json;
                                 bare `approve`/`pair` lists pending,
                                 `pair --wait` auto-approves the next DM
dcbot allow|remove <snowflake> / policy <pairing|allowlist|disabled>
dcbot dm <text> [--to <snowflake>]  DM an allowlisted user via the Discord
                                 API — works without an inbound message
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
- Never use the plugin's `/discord:access` or `/discord:configure` skills
  inside a deployment — they hardcode the global
  `~/.claude/channels/discord` and silently edit files the server never
  reads. Use the `dcbot` equivalents instead.
- `dcbot new`/`register`/`start` write `.claude/rules/dcbot.md` into the
  deployment — Claude Code auto-loads it at session start, so the in-session
  agent already knows these rules.
- One token = one process. Two processes on the same token deliver
  every DM twice.
- Prereqs: `claude` + discord plugin (auto-installed by `dcbot
  new`/`dcbot start` at user scope; manual:
  `claude plugin install discord@claude-plugins-official`), `bun`,
  `tmux` on PATH.
- `dcbot start` pre-accepts Claude Code's folder-trust dialog for the
  deployment dir (`projects[dir].hasTrustDialogAccepted` in
  `~/.claude.json`) — otherwise the first tmux launch hangs on a prompt
  nobody can answer.
- `dcbot start` DMs the first `allowFrom` entry in access.json once the
  session is up ("<name> is online") — skipped in pairing mode (nobody
  to greet); a failed DM only warns, the session stays up.

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
cp -a ~/.claude/channels/discord <dir>/.discord-state
dcbot register <dir> && cd <dir> && dcbot doctor && dcbot start <name>
```

`<dir>` is the bot's existing working dir (create it if needed) —
`register` takes no name; the bot is named after the dir (or its
`bot.toml`). Copy, don't move: `cp -a` preserves the `.env` 0600 perms
and leaves the original as rollback. Stop every claude session using
the global channel first; delete the old global dir after the migrated
bot is verified healthy.
