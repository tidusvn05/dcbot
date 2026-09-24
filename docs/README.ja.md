# dcbot

[`discord@claude-plugins-official`](https://github.com/anthropics/claude-plugins-official/tree/main/external_plugins/discord) のコンパニオン CLI — 1 台のマシンで **複数の Discord チャネルボット (Claude Code 用)** を、グローバル `~/.claude/channels/discord` ではなく各 `.discord-state` でオンボード・運用します。

背景: 公式プラグインは state を単一のグローバルディレクトリに保持し、`/discord:access` スキルはそのパスにハードコードされています。dcbot は各ボットに独立したデプロイディレクトリを与え、`claude --channels …` を実行する tmux セッションを管理し、各ボットのローカル `access.json` に対してアクセス制御 (approve/allow/policy/groups) を直接実行します。

**言語:** English (デフォルト) · Tiếng Việt · 日本語 — `dcbot config set lang <en|vi|ja>` または `--lang` で切替。

## インストール

```bash
curl -fsSL https://raw.githubusercontent.com/tidusvn05/dcbot/main/install.sh | bash
```

バージョン指定: `curl -fsSL …/install.sh | DCBOT_VERSION=v0.1.0 bash`

ソースから: `cargo install --path .` (Rust stable)

**要件:** Claude Code + discord プラグイン (`/plugin install discord@claude-plugins-official`), `tmux`, `bun`。Linux & macOS。

## クイックスタート

```bash
dcbot new helper        # ウィザード: ポータル手順 → トークン → 検証 → allowlist シード
dcbot attach helper     # claude が動く tmux セッションへアタッチ
dcbot list              # マシン上の全ボット
dcbot doctor            # カレント dir のデプロイをヘルスチェック
```

tmux セッション内では claude が `DISCORD_STATE_DIR=<dir>/.discord-state` 付きで動作 — ボットへの DM がそのセッションに届きます。

## コマンド

| Command | 説明 |
| --- | --- |
| `dcbot new <name> [--dir p \| --here]` | オンボードウィザード — dir, `.env` (600), `access.json` シード, `bot.toml`, `run.sh`, `.gitignore` を生成し registry 登録 |
| `dcbot start/stop/restart <name>` | tmux ライフサイクル (`--respawn` で claude 自動再起動) |
| `dcbot attach <name>` | `tmux attach -t dcbot-<name>` |
| `dcbot logs <name> [-f]` | セッション出力を表示 |
| `dcbot list` | registry ⨝ tmux — running/stopped/missing、トークン重複・孤立セッション警告 |
| `dcbot status [name]` | トークン live 検証、ゲートウェイ、allowlist/pending 数 |
| `dcbot doctor [target]` | `.env` 権限、トークン、access.json、ツール、プラグイン、run.sh、トークン重複を検査 |
| `dcbot approve <code>` | ペアリング承認 → `allowFrom` + `approved/<senderId>` マーカー書込 |
| `dcbot deny / allow / remove / policy` | cwd (または名前指定) のボットの `access.json` を管理 |
| `dcbot group add/rm <channelId>` | guild チャンネル opt-in (`--no-mention`, `--allow ids`) |
| `dcbot set <key> <value>` | `ackReaction`, `replyToMode`, `textChunkLimit`, `chunkMode`, `mentionPatterns` |
| `dcbot register <dir> / forget / prune` | registry 管理・ドリフト整理 |
| `dcbot config set lang <en\|vi\|ja>` | UI 言語を保存 |
| `dcbot completions <shell>` | シェル補完 |

デプロイ dir 内では名前引数を省略可能 — 最も近い `.discord-state` を遡って解決します。

## レイアウト

```
<deployment>/                # ディスク上の任意の場所
  bot.toml                   # マニフェスト (bot id/tag, created, channels flag)
  run.sh                     # DISCORD_STATE_DIR export → exec claude --channels …
  .discord-state/
    .env                     # DISCORD_BOT_TOKEN (600)
    access.json              # dmPolicy / allowFrom / groups / pending / delivery config
    approved/  inbox/
  logs/

~/.config/dcbot/
  config.toml                # lang
  bots.toml                  # registry: name → dir インデックス (manifest が正)
```

## アクセスモデル (上流プラグインと同じ semantics)

- `dmPolicy`: `pairing` (デフォルト — 未知の送信者にコードを返す), `allowlist` (サイレント drop), `disabled`.
- `approve` は pending の `senderId` を `allowFrom` に移し、サーバーがポーリングする `approved/<senderId>` マーカーを書き込みます。
- guild チャンネルは **チャンネル** snowflake 単位で opt-in、スレッドは親を継承、`requireMention` はデフォルト true。
- `dcbot new` は snowflake 指定時に `allowlist` をシード — プラグインが推奨するロックダウン状態です。

## License

[MIT](../LICENSE-MIT) または [Apache-2.0](../LICENSE-APACHE) のデュアルライセンス。
