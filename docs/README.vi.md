# dcbot

CLI đồng hành cho [`discord@claude-plugins-official`](https://github.com/anthropics/claude-plugins-official/tree/main/external_plugins/discord) — onboard và vận hành **nhiều Discord-channel bot cho Claude Code** trên một máy, mỗi bot một `.discord-state` riêng thay vì dùng chung global `~/.claude/channels/discord`.

Lý do: plugin gốc giữ state ở một thư mục global và skill `/discord:access` hardcode path đó. dcbot cho mỗi bot một deployment dir riêng, quản lý tmux session chạy `claude --channels …`, và thao tác access-control (approve/allow/policy/groups) trực tiếp trên `access.json` local của từng bot.

**Ngôn ngữ:** English (mặc định) · Tiếng Việt · 日本語 — đổi bằng `dcbot config set lang <en|vi|ja>` hoặc `--lang`.

## Cài đặt

```bash
curl -fsSL https://raw.githubusercontent.com/tidusvn05/dcbot/main/install.sh | bash
```

Pin version: `curl -fsSL …/install.sh | DCBOT_VERSION=v0.1.0 bash`

Hoặc từ source: `cargo install --path .` (Rust stable).

**Yêu cầu:** Claude Code + discord plugin (`/plugin install discord@claude-plugins-official`), `tmux`, `bun`. Linux & macOS.

## Quick start

```bash
dcbot new business-bot        # wizard: hướng dẫn portal → token → validate → seed allowlist
dcbot attach business-bot     # vào tmux session đang chạy claude
dcbot list              # tất cả bot trên máy
dcbot doctor            # health-check deployment ở thư mục hiện tại
```

Trong tmux session, claude chạy với `DISCORD_STATE_DIR=<dir>/.discord-state` — DM bot là đang nói chuyện với session đó.

## Hướng dẫn

### Setup mới — từ `plugin install` tới bot chạy

Yêu cầu: `claude` (Claude Code), `tmux`, `bun`, tài khoản Discord.

```bash
# 1. Cài channel plugin — trong một session claude bất kỳ:
/plugin install discord@claude-plugins-official

# 2. Cài dcbot:
curl -fsSL https://raw.githubusercontent.com/tidusvn05/dcbot/main/install.sh | bash

# 3. Tạo deployment. Wizard in sẵn các bước Developer Portal
#    (New Application → Bot → Reset Token → bật Message Content Intent
#    → invite URL), validate token trực tiếp, seed access.json với
#    Discord snowflake của bạn (allowlist mode — không cần pairing),
#    ghi run.sh, và đăng ký bot:
dcbot new business-bot                 # tạo ./business-bot/
dcbot new business-bot --dir ~/bots/x  # hoặc path chỉ định
dcbot new business-bot --here          # hoặc deploy ngay tại cwd

# 4. Chạy:
dcbot start business-bot               # tmux session dcbot-business-bot
dcbot attach business-bot              # vào session claude
```

DM bot — nếu đã seed snowflake thì dùng được ngay. Nếu để trống (pairing mode), bot trả lời bằng code; duyệt bằng `dcbot approve <code>` (chạy trong deployment dir hoặc kèm tên bot).

### Migrate từ bot global có sẵn

Nếu bot đang chạy với state dir global (`~/.claude/channels/discord/`), chuyển sang dcbot — layout bên trong giống hệt nên chỉ cần một `mv`:

```bash
# 1. Dừng mọi session claude đang dùng global channel. Một token không
#    được chạy ở 2 process — mọi DM sẽ bị deliver trùng.

# 2. Move cả state dir vào deployment dir (.env, access.json,
#    approved/, inbox/ — giữ nguyên allowlist, groups, pending):
mkdir -p ~/bots/mybot
mv ~/.claude/channels/discord ~/bots/mybot/.discord-state

# 3. Nhận nuôi — validate token hiện có, ghi bot.toml + run.sh,
#    đăng ký registry:
dcbot register ~/bots/mybot
cd ~/bots/mybot && dcbot doctor   # kiểm tra deployment ổn

# 4. Từ nay chỉ start bot qua dcbot:
dcbot start mybot
```

Sau khi move, **đừng** chạy `claude --channels …` thủ công nữa — thiếu `DISCORD_STATE_DIR` thì server rơi về global dir (giờ trống) và exit vì thiếu token. Nếu bạn đã copy (thay vì move) state dir, xóa `~/.claude/channels/discord` sau khi bot mới chạy ổn, tránh session lạc làm sống lại vấn đề trùng token.

## Dùng với AI agent

Copy block dưới vào context của agent (`AGENTS.md`, CLAUDE.md, hoặc gửi
thẳng trong chat). Sau đó chỉ cần mô tả yêu cầu — agent đã biết toàn bộ
command surface.

```text
dcbot manages Discord-channel bots for Claude Code on this machine.
Every bot lives in its own deployment dir containing .discord-state/
(never touch the global ~/.claude/channels/discord). All commands are
scriptable / non-interactive-safe:

  dcbot new <name> --yes [--dir <path>|--here]
      Onboard a bot. Token comes from $DCBOT_BOT_TOKEN or --token <t>;
      --owner <snowflake> seeds the allowlist (recommended; empty
      leaves pairing mode), --start launches the tmux session.
      If the user has no token yet, print these steps and wait:
      Discord Developer Portal → New Application → Bot → Reset Token →
      enable Message Content Intent → OAuth2 URL Generator (scope bot;
      perms: View Channels, Send Messages, Send Messages in Threads,
      Read Message History, Attach Files, Add Reactions) → invite the
      bot to a shared server. The channel plugin must be installed in
      Claude Code first: /plugin install discord@claude-plugins-official
  dcbot register <dir>     adopt an existing deployment (has bot.toml
                           or .discord-state); generates run.sh if missing
  dcbot list / status [name] / doctor [name|dir]
  dcbot start|stop|restart <name> [--respawn] / attach <name> / logs <name> [-f]
  dcbot approve|deny <pairing-code> — codes live in .discord-state/access.json
  dcbot allow|remove <snowflake> / policy <pairing|allowlist|disabled>
  dcbot group add <channelId> [--no-mention] [--allow id1,id2] / group rm <channelId>
  dcbot set <key> <value> — ackReaction, replyToMode, textChunkLimit,
      chunkMode, mentionPatterns
  dcbot forget <name> / prune — registry cleanup (never deletes dirs)

Inside a deployment dir, <name> may be omitted (resolved via the
nearest .discord-state upward). To migrate a global bot:
  mv ~/.claude/channels/discord <dir>/.discord-state
  dcbot register <dir> && cd <dir> && dcbot doctor && dcbot start <name>
Never launch `claude --channels` outside `dcbot start` — without
DISCORD_STATE_DIR the channel server targets the global dir, and one
token in two processes duplicates every DM.
```

Ví dụ câu lệnh mô tả:

- "tạo bot mới tên business-bot, token là ..."
- "migrate bot cũ ở ~/.claude/channels/discord sang dcbot, đặt tên legacy-bot"
- "bot nào đang chạy?"
- "approve pairing code a4f91c cho business-bot"
- "check sức khỏe deployment ở dir này"

## Commands

| Command | Chức năng |
| --- | --- |
| `dcbot new <name> [--dir p \| --here \| --yes]` | Wizard onboarding — tạo dir, `.env` (600), seed `access.json`, `bot.toml`, `run.sh`, `.gitignore`; đăng ký registry. `--yes` chạy non-interactive (`--token`/`$DCBOT_BOT_TOKEN`, `--owner`, `--start`) — cho agent |
| `dcbot start/stop/restart <name>` | tmux lifecycle (`--respawn` tự restart claude khi exit) |
| `dcbot attach <name>` | `tmux attach -t dcbot-<name>` |
| `dcbot logs <name> [-f]` | Xem output session |
| `dcbot list` | Registry ⨝ tmux — running/stopped/missing, cảnh báo trùng token & session mồ côi |
| `dcbot status [name]` | Check token live, gateway, số allowlist/pending |
| `dcbot doctor [target]` | Kiểm tra `.env` perms, token, access.json, tools, plugin, run.sh, trùng token |
| `dcbot approve <code>` | Duyệt pairing → `allowFrom` + ghi marker `approved/<senderId>` |
| `dcbot deny / allow / remove / policy` | Quản `access.json` của bot ở cwd (hoặc theo tên) |
| `dcbot group add/rm <channelId>` | Opt-in guild channel (`--no-mention`, `--allow ids`) |
| `dcbot set <key> <value>` | `ackReaction`, `replyToMode`, `textChunkLimit`, `chunkMode`, `mentionPatterns` |
| `dcbot register <dir> / forget / prune` | Quản lý registry & dọn drift |
| `dcbot config set lang <en\|vi\|ja>` | Đổi ngôn ngữ UI |
| `dcbot completions <shell>` | Shell completions |

Tham số tên có thể bỏ trống khi đứng trong deployment dir — dcbot tự resolve bằng cách đi lên tới `.discord-state` gần nhất.

## Layout

```
<deployment>/                # bất cứ đâu trên đĩa
  bot.toml                   # manifest (bot id/tag, created, channels flag)
  run.sh                     # export DISCORD_STATE_DIR → exec claude --channels …
  .discord-state/
    .env                     # DISCORD_BOT_TOKEN (600)
    access.json              # dmPolicy / allowFrom / groups / pending / delivery config
    approved/  inbox/
  logs/

~/.config/dcbot/
  config.toml                # lang
  bots.toml                  # registry: name → dir (manifest mới là source of truth)
```

## Access model (cùng semantics với plugin gốc)

- `dmPolicy`: `pairing` (mặc định — người lạ nhận code), `allowlist` (drop lặng), `disabled`.
- `approve` chuyển `senderId` pending vào `allowFrom` và ghi marker `approved/<senderId>` mà server poll.
- Guild channel opt-in theo **channel** snowflake; thread kế thừa parent; `requireMention` mặc định true.
- `dcbot new` seed sẵn `allowlist` với snowflake của bạn — đúng khuyến nghị lockdown của plugin.

## License

[MIT](../LICENSE-MIT) hoặc [Apache-2.0](../LICENSE-APACHE), tùy chọn.
