# dcbot

CLI đồng hành cho [`discord@claude-plugins-official`](https://github.com/anthropics/claude-plugins-official/tree/main/external_plugins/discord) — onboard và vận hành **nhiều Discord-channel bot cho Claude Code** trên một máy, mỗi bot một `.discord-state` riêng thay vì dùng chung global `~/.claude/channels/discord`.

Lý do: plugin gốc giữ state ở một thư mục global và skill `/discord:access` hardcode path đó. dcbot cho mỗi bot một deployment dir riêng, quản lý tmux session chạy `claude --channels …`, và thao tác access-control (approve/allow/policy/groups) trực tiếp trên `access.json` local của từng bot.

**Ngôn ngữ:** English (mặc định) · Tiếng Việt · 日本語 — đổi bằng `dcbot config set lang <en|vi|ja>` hoặc `--lang`.

## Cài đặt

```bash
curl -fsSL https://raw.githubusercontent.com/tidusvn05/dcbot/main/install.sh | bash
```

**Yêu cầu:** Claude Code + discord plugin (`dcbot` tự cài), `tmux`, `bun`. Linux & macOS.

## Quick start

```bash
dcbot new business-bot        # wizard: hướng dẫn portal → token → validate → seed allowlist
dcbot attach business-bot     # vào tmux session đang chạy claude
dcbot list              # tất cả bot trên máy
dcbot doctor            # health-check deployment ở thư mục hiện tại
```

Trong tmux session, claude chạy với `DISCORD_STATE_DIR=<dir>/.discord-state` — DM bot là đang nói chuyện với session đó.

## Hướng dẫn

### Agent-first — giao cho agent làm

dcbot tự ship usage contract — `dcbot agents.md` in nó ra (cũng nằm ở
[`AGENTS.md`](../AGENTS.md)). Chỉ cần trỏ agent tới nó rồi mô tả yêu cầu:

```text
follow cli `dcbot agents.md`

bot nào đang chạy?
```

```text
follow cli `dcbot agents.md`

tạo bot mới tên business-bot, token là ...
```

> **Token trong prompt** sẽ đi qua model provider của agent đúng 1 lần.
> dcbot chỉ gửi token tới `api.discord.com` (để validate) rồi ghi vào
> `.discord-state/.env` mode `0600` — không endpoint public nào khác thấy
> nó. Muốn token không qua chat? Tự chạy `dcbot new business-bot
> --yes` trong terminal — nó chỉ hỏi token (nhập ẩn), sau đó agent điều
> khiển tiếp bằng các lệnh `dcbot` khác.

```text
follow cli `dcbot agents.md`

migrate bot cũ ở ~/.claude/channels/discord sang dcbot, đặt tên legacy-bot
```

```text
follow cli `dcbot agents.md`

approve pairing code a4f91c cho business-bot
```

```text
follow cli `dcbot agents.md`

check sức khỏe deployment ở dir này
```

Hỏi gì cũng được — phần sau dòng đầu là yêu cầu của bạn.

### Setup mới — từ con số 0 tới bot chạy

Yêu cầu: `claude` (Claude Code), `tmux`, `bun`, tài khoản Discord.

```bash
# 1. Cài dcbot:
curl -fsSL https://raw.githubusercontent.com/tidusvn05/dcbot/main/install.sh | bash

# 2. Tạo deployment. Wizard in sẵn các bước Developer Portal
#    (New Application → Bot → Reset Token → bật Message Content Intent),
#    validate token trực tiếp, tự sinh invite URL (không cần OAuth2
#    URL Generator), tự cài discord channel plugin, seed access.json
#    với Discord snowflake của bạn (allowlist mode — không cần pairing),
#    ghi run.sh, và đăng ký bot:
dcbot new business-bot                 # tạo ./business-bot/
dcbot new business-bot --dir ~/bots/x  # hoặc path chỉ định
dcbot new business-bot --here          # hoặc deploy ngay tại cwd

# 3. Chạy — tự cài plugin, greet owner qua DM và mọi channel đã cấu
#    hình khi session lên (không có gì cấu hình: bỏ qua):
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

## Commands

| Command | Chức năng |
| --- | --- |
| `dcbot new <name> [--dir p \| --here \| --yes]` | Wizard onboarding — tạo dir, `.env` (600), seed `access.json`, `bot.toml`, `run.sh`, `.gitignore`; đăng ký registry. `--yes` chạy non-interactive (`--token`/`$DCBOT_BOT_TOKEN`, `--owner`, `--start`) — cho agent |
| `dcbot start/stop/restart <name>` | tmux lifecycle (`--respawn` tự restart claude khi exit) |
| `dcbot attach <name>` | `tmux attach -t dcbot-<name>` |
| `dcbot logs <name> [-f]` | Xem output session |
| `dcbot list` | Registry ⨝ tmux — running/stopped/missing, cảnh báo trùng token & session mồ côi |
| `dcbot status [name]` | Check token live, gateway, số allowlist/pending |
| `dcbot invite [name] [--open\|--copy]` | In lại OAuth2 invite URL; `--open` mở browser, `--copy` copy clipboard |
| `dcbot doctor [target]` | Kiểm tra `.env` perms, token, access.json, tools, plugin, run.sh, trùng token |
| `dcbot approve <code>` | Duyệt pairing → `allowFrom` + ghi marker `approved/<senderId>` |
| `dcbot deny / allow / remove / policy` | Quản `access.json` của bot ở cwd (hoặc theo tên) |
| `dcbot group add/rm <channelId>` | Opt-in guild channel (`--no-mention`, `--allow ids`) |
| `dcbot set <key> <value>` | `ackReaction`, `replyToMode`, `textChunkLimit`, `chunkMode`, `mentionPatterns` |
| `dcbot dm <text> [--to id]` | DM user trong allowlist qua Discord API — không cần tin nhắn inbound |
| `dcbot register <dir> / forget / prune` | Quản lý registry & dọn drift |
| `dcbot config set lang <en\|vi\|ja>` | Đổi ngôn ngữ UI |
| `dcbot completions <shell>` | Shell completions |

Tham số tên có thể bỏ trống khi đứng trong deployment dir — dcbot tự resolve bằng cách đi lên tới `.discord-state` gần nhất.

## Layout

```
<deployment>/                # bất cứ đâu trên đĩa
  bot.toml                   # manifest (bot id/tag, app id, created, channels flag)
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

## Tùy chọn cài đặt khác

Pin version:

```bash
curl -fsSL https://raw.githubusercontent.com/tidusvn05/dcbot/main/install.sh | DCBOT_VERSION=v0.3.0 bash
```

Từ source (Rust stable):

```bash
cargo install --path .
```

## License

[MIT](../LICENSE-MIT) hoặc [Apache-2.0](../LICENSE-APACHE), tùy chọn.
