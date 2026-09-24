# dcbot

[`discord@claude-plugins-official`](https://github.com/anthropics/claude-plugins-official/tree/main/external_plugins/discord) のコンパニオン CLI — 1 台のマシンで **複数の Discord チャネルボット (Claude Code 用)** を、グローバル `~/.claude/channels/discord` ではなく各 `.discord-state` でオンボード・運用します。

背景: 公式プラグインは state を単一のグローバルディレクトリに保持し、`/discord:access` スキルはそのパスにハードコードされています。dcbot は各ボットに独立したデプロイディレクトリを与え、`claude --channels …` を実行する tmux セッションを管理し、各ボットのローカル `access.json` に対してアクセス制御 (approve/allow/policy/groups) を直接実行します。

**言語:** English (デフォルト) · Tiếng Việt · 日本語 — `dcbot config set lang <en|vi|ja>` または `--lang` で切替。

## インストール

```bash
curl -fsSL https://raw.githubusercontent.com/tidusvn05/dcbot/main/install.sh | bash
```

**要件:** Claude Code + discord プラグイン (`/plugin install discord@claude-plugins-official`), `tmux`, `bun`。Linux & macOS。

## クイックスタート

```bash
dcbot new business-bot        # ウィザード: ポータル手順 → トークン → 検証 → allowlist シード
dcbot attach business-bot     # claude が動く tmux セッションへアタッチ
dcbot list              # マシン上の全ボット
dcbot doctor            # カレント dir のデプロイをヘルスチェック
```

tmux セッション内では claude が `DISCORD_STATE_DIR=<dir>/.discord-state` 付きで動作 — ボットへの DM がそのセッションに届きます。

## ガイド

### Agent-first — エージェントに任せる

dcbot は usage contract を同梱しています — `dcbot agents.md` で出力
(リポジトリの [`AGENTS.md`](../AGENTS.md) と同一)。エージェントに読ませて
から、やりたいことを伝えるだけです:

```text
read `dcbot agents.md`

今動いているボットは?
```

```text
read `dcbot agents.md`

business-bot という名前で新しいボットを作って、トークンは ...
```

> **プロンプト内のトークン** はエージェントのモデルプロバイダを1回だけ
> 通過します。dcbot 自体は `api.discord.com` (検証) にのみ送信し、
> `.discord-state/.env` (モード `0600`) に保存します — 他の公開
> エンドポイントには一切送られません。チャットに出したくない場合は、自分で
> 端末で `dcbot new business-bot --yes` を実行 — トークンだけを (非表示で)
> 聞かれ、あとはエージェントが `dcbot` コマンドで引き継ぎます。

```text
read `dcbot agents.md`

~/.claude/channels/discord の既存ボットを legacy-bot として dcbot に移行して
```

```text
read `dcbot agents.md`

business-bot で pairing code a4f91c を承認して
```

```text
read `dcbot agents.md`

このディレクトリのデプロイを doctor して
```

何を聞いても構いません — 1行目以降はあなたの依頼です。

### 新規セットアップ — `plugin install` から起動まで

要件: `claude` (Claude Code), `tmux`, `bun`, Discord アカウント。

```bash
# 1. チャネルプラグインをインストール — 任意の claude セッション内で:
/plugin install discord@claude-plugins-official

# 2. dcbot をインストール:
curl -fsSL https://raw.githubusercontent.com/tidusvn05/dcbot/main/install.sh | bash

# 3. デプロイを作成。ウィザードが Developer Portal の手順を表示
#    (New Application → Bot → Reset Token → Message Content Intent
#    有効化)、トークンを live 検証し、招待 URL を自動生成 (OAuth2
#    URL Generator 不要)、あなたの Discord snowflake で access.json
#    をシード (allowlist モード — pairing 不要)、run.sh を生成して
#    registry に登録します:
dcbot new business-bot                 # ./business-bot/ を作成
dcbot new business-bot --dir ~/bots/x  # または任意のパス
dcbot new business-bot --here          # またはカレント dir にデプロイ

# 4. 起動:
dcbot start business-bot               # tmux セッション dcbot-business-bot
dcbot attach business-bot              # claude セッションにアタッチ
```

ボットに DM — snowflake をシード済みならそのまま使えます。空のまま (pairing モード) なら、ボットがコードを返信します。`dcbot approve <code>` で承認 (デプロイ dir 内で実行、またはボット名を指定)。

### 既存のグローバルボットからの移行

グローバル state dir (`~/.claude/channels/discord/`) で動いているボットを dcbot 配下へ移します — 内部レイアウトは同一なので `mv` 一発です:

```bash
# 1. グローバル channel を使う claude セッションを全て停止。同一
#    トークンを 2 プロセスで動かすと全 DM が二重配送されます。

# 2. state dir をデプロイ dir へ丸ごと移動 (.env, access.json,
#    approved/, inbox/ — allowlist, groups, pending すべて保持):
mkdir -p ~/bots/mybot
mv ~/.claude/channels/discord ~/bots/mybot/.discord-state

# 3. 引き取り — 既存トークンを検証、bot.toml + run.sh を生成、
#    registry に登録:
dcbot register ~/bots/mybot
cd ~/bots/mybot && dcbot doctor   # デプロイが健全か確認

# 4. 以後は dcbot 経由でのみ起動:
dcbot start mybot
```

移行後は `claude --channels …` を手動で起動しないでください — `DISCORD_STATE_DIR` が無いとサーバーは (空になった) グローバル dir にフォールバックし、トークン不在で exit します。state dir を move ではなくコピーした場合は、移行先が健全と確認でき次第 `~/.claude/channels/discord` を削除し、迷子セッションによるトークン重複問題の再発を防いでください。

## コマンド

| Command | 説明 |
| --- | --- |
| `dcbot new <name> [--dir p \| --here \| --yes]` | オンボードウィザード — dir, `.env` (600), `access.json` シード, `bot.toml`, `run.sh`, `.gitignore` を生成し registry 登録 (`--yes` で非対話モード: `--token`/`$DCBOT_BOT_TOKEN`, `--owner`, `--start` — エージェント向け) |
| `dcbot start/stop/restart <name>` | tmux ライフサイクル (`--respawn` で claude 自動再起動) |
| `dcbot attach <name>` | `tmux attach -t dcbot-<name>` |
| `dcbot logs <name> [-f]` | セッション出力を表示 |
| `dcbot list` | registry ⨝ tmux — running/stopped/missing、トークン重複・孤立セッション警告 |
| `dcbot status [name]` | トークン live 検証、ゲートウェイ、allowlist/pending 数 |
| `dcbot invite [name]` | OAuth2 招待 URL を再表示 (ボットを別サーバーに追加) |
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
  bot.toml                   # マニフェスト (bot id/tag, app id, created, channels flag)
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

## その他のインストール方法

バージョン指定:

```bash
curl -fsSL https://raw.githubusercontent.com/tidusvn05/dcbot/main/install.sh | DCBOT_VERSION=v0.3.0 bash
```

ソースから (Rust stable):

```bash
cargo install --path .
```

## License

[MIT](../LICENSE-MIT) または [Apache-2.0](../LICENSE-APACHE) のデュアルライセンス。
