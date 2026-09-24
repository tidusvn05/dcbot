# dcbot

[`discord@claude-plugins-official`](https://github.com/anthropics/claude-plugins-official/tree/main/external_plugins/discord) のコンパニオン CLI — 1 台のマシンで **複数の Discord チャネルボット (Claude Code 用)** を、グローバル `~/.claude/channels/discord` ではなく各 `.discord-state` でオンボード・運用します。

背景: 公式プラグインは state を単一のグローバルディレクトリに保持し、`/discord:access` スキルはそのパスにハードコードされています。dcbot は各ボットに独立したデプロイディレクトリを与え、`claude --channels …` を実行する tmux セッションを管理し、各ボットのローカル `access.json` に対してアクセス制御 (approve/allow/policy/groups) を直接実行します。

**言語:** English (デフォルト) · Tiếng Việt · 日本語 — `dcbot config set lang <en|vi|ja>` または `--lang` で切替。

## インストール

```bash
curl -fsSL https://raw.githubusercontent.com/tidusvn05/dcbot/main/install.sh | bash
```

**要件:** Claude Code + discord プラグイン (`dcbot` が自動インストール), `tmux`, `bun`。Linux & macOS。

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

dcbot は usage contract を同梱しています — `dcbot agent` で出力
(リポジトリの [`AGENTS.md`](../AGENTS.md) と同一)。エージェントに読ませて
から、やりたいことを伝えるだけです:

```text
follow cli `dcbot agent`

今動いているボットは?
```

```text
follow cli `dcbot agent`

business-bot という名前で新しいボットを作って、トークンは ...
```

> **プロンプト内のトークン** はエージェントのモデルプロバイダを1回だけ
> 通過します。dcbot 自体は `api.discord.com` (検証) にのみ送信し、
> `.discord-state/.env` (モード `0600`) に保存します — 他の公開
> エンドポイントには一切送られません。チャットに出したくない場合は、自分で
> 端末で `dcbot new business-bot --yes` を実行 — トークンだけを (非表示で)
> 聞かれ、あとはエージェントが `dcbot` コマンドで引き継ぎます。

```text
follow cli `dcbot agent`

既存の discord ボットを dcbot に移行して — path は ~/existing-bot
```

```text
follow cli `dcbot agent`

business-bot で pairing code a4f91c を承認して
```

```text
follow cli `dcbot agent`

このディレクトリのデプロイを doctor して
```

何を聞いても構いません — 1行目以降はあなたの依頼です。

### 新規セットアップ — ゼロから起動まで

要件: `claude` (Claude Code), `tmux`, `bun`, Discord アカウント。

```bash
# 1. dcbot をインストール:
curl -fsSL https://raw.githubusercontent.com/tidusvn05/dcbot/main/install.sh | bash

# 2. デプロイを作成。ウィザードが Developer Portal の手順を表示
#    (New Application → Bot → Reset Token → Message Content Intent
#    有効化)、トークンを live 検証し、招待 URL を自動生成 (OAuth2
#    URL Generator 不要)、discord チャネルプラグインを自動インストールし、
#    あなたの Discord snowflake で access.json をシード (allowlist
#    モード — pairing 不要)、run.sh を生成して registry に登録します:
dcbot new business-bot                 # ./business-bot/ を作成
dcbot new business-bot --dir ~/bots/x  # または任意のパス
dcbot new business-bot --here          # またはカレント dir にデプロイ

# 3. 起動 — プラグインを自動インストールし、セッションが立ち上がると
#    owner への DM と設定済みの全チャンネルに greeting を送信
#    (設定なし: スキップ):
dcbot start business-bot               # tmux セッション dcbot-business-bot
dcbot attach business-bot              # claude セッションにアタッチ
```

ボットに DM — snowflake をシード済みならそのまま使えます。空のまま (pairing モード) なら、ボットがコードを返信します。`dcbot approve <code>` で承認 (デプロイ dir 内で実行、またはボット名を指定) — あるいは先に `dcbot pair --wait` を実行してから DM すれば自動承認。ボットの返信にある `/discord:access pair` の案内は無視してください — dcbot デプロイではあの skill はこのボットのではなく global の state dir を書き換えます。

### 既存のグローバルボットからの移行

グローバル state dir (`~/.claude/channels/discord/`) で動いているボットを dcbot 配下へ — 内部レイアウトは同一なので、ボットの既存 dir へコピーするだけです (無ければ作成):

```bash
# 1. グローバル channel を使う claude セッションを全て停止。同一
#    トークンを 2 プロセスで動かすと全 DM が二重配送されます。

# 2. state dir をボットの dir に .discord-state としてコピー
#    (.env, access.json, approved/, inbox/ — allowlist, groups,
#    pending すべて保持。-a でトークンの 0600 パーミッションも維持):
cp -a ~/.claude/channels/discord ~/existing-bot/.discord-state

# 3. 引き取り — 既存トークンを検証、bot.toml + run.sh を生成、
#    dir 名 (ここでは existing-bot) で registry に登録:
dcbot register ~/existing-bot
cd ~/existing-bot && dcbot doctor   # デプロイが健全か確認

# 4. 以後は dcbot 経由でのみ起動:
dcbot start existing-bot
```

`claude --channels …` を手動で起動しないでください — `DISCORD_STATE_DIR` が無いとサーバーはグローバル dir にフォールバックします。コピー (move ではなく) したので、移行先が健全と確認でき次第 `~/.claude/channels/discord` を削除し、迷子セッションによるトークン重複の再発を防いでください。

## コマンド

| Command | 説明 |
| --- | --- |
| `dcbot new <name> [--dir p \| --here \| --yes]` | オンボードウィザード — dir, `.env` (600), `access.json` シード, `bot.toml`, `run.sh`, `.gitignore` を生成し registry 登録 (`--yes` で非対話モード: `--token`/`$DCBOT_BOT_TOKEN`, `--owner`, `--start`, `--pair` — エージェント向け) |
| `dcbot start/stop/restart <name>` | tmux ライフサイクル (`--respawn` で claude 自動再起動) |
| `dcbot attach <name>` | `tmux attach -t dcbot-<name>` |
| `dcbot logs <name> [-f]` | セッション出力を表示 |
| `dcbot list` | registry ⨝ tmux — running/stopped/missing、トークン重複・孤立セッション警告 |
| `dcbot status [name]` | トークン live 検証、ゲートウェイ、allowlist/pending 数 |
| `dcbot invite [name] [--open\|--copy]` | OAuth2 招待 URL を再表示; `--open` でブラウザ起動、`--copy` でクリップボードへ |
| `dcbot doctor [target]` | `.env` 権限、トークン、access.json、ツール、プラグイン、run.sh、トークン重複を検査 |
| `dcbot approve [code]` / `dcbot pair [--wait]` | ペアリング承認 → `allowFrom` + `approved/<senderId>` マーカー書込。code なしで pending 一覧、`--wait` で次の DM のコードを自動承認 |
| `dcbot deny / allow / remove / policy` | cwd (または名前指定) のボットの `access.json` を管理 |
| `dcbot group add/rm <channelId>` | guild チャンネル opt-in (`--no-mention`, `--allow ids`) |
| `dcbot set <key> <value>` | `ackReaction`, `replyToMode`, `textChunkLimit`, `chunkMode`, `mentionPatterns` |
| `dcbot dm <text> [--to id]` | allowlist 内のユーザーへ Discord API 経由で DM — inbound メッセージ不要 |
| `dcbot register <dir> / forget / prune` | registry 管理・ドリフト整理 |
| `dcbot config set lang <en\|vi\|ja>` | UI 言語を保存 |
| `dcbot completions <shell>` | シェル補完 |

デプロイ dir 内では名前引数を省略可能 — 最も近い `.discord-state` を遡って解決します。

## レイアウト

```
<deployment>/                # ディスク上の任意の場所
  bot.toml                   # マニフェスト (bot id/tag, app id, created, channels flag)
  run.sh                     # DISCORD_STATE_DIR export → exec claude --channels …
  .claude/rules/dcbot.md     # セッション rules — Claude Code が自動ロード
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

### プラグインの `/discord:*` スキルはここでは使わない

プラグインが同梱する 2 つのスキル — `/discord:access` と
`/discord:configure` — はグローバル dir `~/.claude/channels/discord` に
ハードコードされています。dcbot デプロイではサーバーが
`<dir>/.discord-state` を読むため、これらのスキルは誰も読まないファイルを
書き換えます: そこで承認したペアリングは完了せず、policy 変更は静かに
無効になり、`configure` はトークンを間違った場所に書きます。ボットの
「Pairing required — run /discord:access pair \<code\>」という返信が
最もハマりやすい箇所です — 必ず dcbot 側で承認してください:

| プラグインのスキル | 対応する dcbot コマンド |
| --- | --- |
| `/discord:access pair <code>` | `dcbot approve <code>` — または DM 前に `dcbot pair --wait` |
| `/discord:access deny <code>` | `dcbot deny <code>` |
| `/discord:access allow\|remove <id>` | `dcbot allow\|remove <id>` |
| `/discord:access policy <mode>` | `dcbot policy <mode>` |
| `/discord:access group add\|rm` | `dcbot group add\|rm` |
| `/discord:access set <k> <v>` | `dcbot set <k> <v>` |
| `/discord:access` (status) | `dcbot status` |
| `/discord:configure <token>` | トークンは `.discord-state/.env` — `dcbot new`/`register` が書込 |

MCP ツール (`reply`, `react`, `edit_message`, `fetch_messages`,
`download_attachment`) は影響なし — サーバー内で動作し、
`DISCORD_STATE_DIR` を尊重します。

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
