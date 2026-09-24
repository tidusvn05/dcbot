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
dcbot new helper        # wizard: hướng dẫn portal → token → validate → seed allowlist
dcbot attach helper     # vào tmux session đang chạy claude
dcbot list              # tất cả bot trên máy
dcbot doctor            # health-check deployment ở thư mục hiện tại
```

Trong tmux session, claude chạy với `DISCORD_STATE_DIR=<dir>/.discord-state` — DM bot là đang nói chuyện với session đó.

## Commands

| Command | Chức năng |
| --- | --- |
| `dcbot new <name> [--dir p \| --here]` | Wizard onboarding — tạo dir, `.env` (600), seed `access.json`, `bot.toml`, `run.sh`, `.gitignore`; đăng ký registry |
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
