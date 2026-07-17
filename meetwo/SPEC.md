# meetwo 仕様書

Minecraftサーバーのログ内容をDiscordに転送し、Discord上のメッセージをMinecraftサーバーのチャットに中継するbot。RCON経由でサーバー状態の取得・操作も行う。

## 1. 概要

- 1つのDiscordチャンネルにつき1つのMinecraftサーバーを紐付けて監視する。
- 対応台数は固定ではなく、環境変数の設定によって何台でも追加できる。
- 言語/フレームワーク: Rust, [poise](https://github.com/serenity-rs/poise)(Discordコマンドフレームワーク、serenity上に構築)
- RCON通信には外部コマンド [`mcrcon`](https://github.com/Tiiffi/mcrcon) を使用する(Dockerイメージ内でビルド・インストール)。

## 2. アーキテクチャ

```
Minecraftサーバー(複数台)
   │ ログファイル (latest.log等)         │ RCON (mcrcon)
   ▼                                      ▲
┌─────────────────────────────────────────────┐
│                  meetwo (bot)                │
│                                               │
│  write_discord タスク (サーバー台数分)        │
│    - ログファイルの追記分を1秒ごとに監視      │
│    - 正規表現にマッチしたら整形してDiscordへ  │
│                                               │
│  event_handler                                │
│    - Discordメッセージを検知                  │
│    - 対応サーバーへ rcon "say ユーザー名: 本文"│
│                                               │
│  スラッシュコマンド (/pong, /list, /status)   │
└─────────────────────────────────────────────┘
   │
   ▼
Discordチャンネル(サーバーごとに1つ)
```

### 主要モジュール

| ファイル | 役割 |
|---|---|
| `src/main.rs` | エントリポイント。ログ監視タスクの起動、Discordイベントハンドラ、bot初期化 |
| `src/server_config.rs` | 環境変数からサーバー設定(`ServerConfig`)一覧を読み込む |
| `src/rcon.rs` | `mcrcon`コマンドを実行する汎用ラッパー(`rcon::run`) |
| `src/commands/help.rs` | `/pong` コマンド(bot説明を返す) |
| `src/commands/list.rs` | `/list` コマンド(コマンドを打ったチャンネルに紐づくサーバーのプレイヤー一覧を返す) |
| `src/commands/status.rs` | `/status` コマンド(全サーバーの稼働状況を一覧表示) |
| `src/commands/stop.rs` | `/stop` コマンド(コマンドを打ったチャンネルに紐づくサーバーを停止する) |

## 3. データ構造

### `ServerConfig`(`server_config.rs`)

```rust
struct ServerConfig {
    name: String,             // 表示用サーバー名
    channel_id: ChannelId,    // 紐づくDiscordチャンネルID
    log_path: String,         // 監視するログファイルの絶対パス
    rcon_host: String,        // RCON接続先ホスト
    rcon_port: u16,           // RCON接続先ポート
    rcon_password: String,    // RCONパスワード
}
```

`SERVER_COUNT` の数だけ `SERVER_{n}_*` という環境変数から読み込まれる(後述)。1台につき1つの`ServerConfig`、1つのDiscordチャンネル、1つのログファイル、1つのRCON接続先が対応する。

### `Data`(poiseのユーザーデータ)

```rust
struct Data {
    servers: Arc<Vec<ServerConfig>>,
}
```

全コマンド・イベントハンドラから`ctx.data().servers`として参照できる。

## 4. 環境変数(`.env`)

| 変数名 | 必須 | 説明 |
|---|---|---|
| `DISCORD_TOKEN` | ○ | Discord botトークン |
| `SERVER_COUNT` | ○ | 監視するMinecraftサーバーの台数 |
| `SERVER_{n}_NAME` | ○ | n番目のサーバーの表示名(`/status`等に使用) |
| `SERVER_{n}_CHANNEL_ID` | ○ | n番目のサーバーに紐づくDiscordチャンネルID |
| `SERVER_{n}_LOG_PATH` | ○ | n番目のサーバーのログファイルの絶対パス(コンテナ内パス) |
| `SERVER_{n}_RCON_HOST` | ○ | n番目のサーバーのRCON接続先ホスト |
| `SERVER_{n}_RCON_PORT` | ○ | n番目のサーバーのRCON接続先ポート |
| `SERVER_{n}_RCON_PASSWORD` | ○ | n番目のサーバーのRCONパスワード |

`n`は1始まりで`SERVER_COUNT`まで連番。いずれか1つでも欠けている場合、bot起動時に`panic`して終了する(フェイルファスト)。

### 設定例(2台構成)

```env
DISCORD_TOKEN=xxxxxxxx

SERVER_COUNT=2

SERVER_1_NAME=server1
SERVER_1_CHANNEL_ID=1517703749118853250
SERVER_1_LOG_PATH=/app/logs_1/latest.log
SERVER_1_RCON_HOST=host.docker.internal
SERVER_1_RCON_PORT=25577
SERVER_1_RCON_PASSWORD=null

SERVER_2_NAME=server2
SERVER_2_CHANNEL_ID=xxxxxxxxxxxxxxxxxxx
SERVER_2_LOG_PATH=/app/logs_2/latest.log
SERVER_2_RCON_HOST=host.docker.internal
SERVER_2_RCON_PORT=25576
SERVER_2_RCON_PASSWORD=null
```

## 5. 機能仕様

### 5.1 ログ監視 → Discord転送(`write_discord`)

- サーバーごとに1つの非同期タスクとして起動する。
- 1秒間隔でログファイルの末尾からの追記分のみを読み取る(前回読み取り位置をバイトオフセットとして保持)。
  - ファイルサイズが前回より縮小していた場合(ログローテーション等)はオフセットを0にリセットして最初から読み直す。
  - 改行で終わっていない末尾の行は次回の読み取りまで保持し、完成してから処理する。
- 追記された各行に対して、以下の正規表現パターンを先頭から順に評価し、最初にマッチしたものだけをDiscordに送信する。

| パターン | マッチ対象 | Discord送信内容 |
|---|---|---|
| `re_rcon` | `[Not Secure] [Rcon] ...` | キャプチャ内容をそのまま |
| `re_start` | `Dedicated server took N seconds to load` | `サーバー起動完了(N秒)` |
| `re_stop` | `Stopping (the )?server` | `サーバー終了` |
| `re_advancement` | 実績解除・目標達成・チャレンジ完了ログ | `実績解除:内容` |
| `re_death` | 死亡ログ(died/was slain/fell/drowned/burned/blew up) | キャプチャ内容をそのまま |
| `re_chat` | チャット発言、join/leaveログ | キャプチャ内容をそのまま |

- Discordへの送信に失敗した場合はパニックせず、標準エラー出力にログを出して処理を継続する。

### 5.2 Discord → Minecraftチャット中継(`event_handler`)

- bot自身の発言は無視する。
- メッセージが投稿されたチャンネルIDと一致する`ServerConfig`を探す。見つからなければ何もしない。
- 見つかった場合、対象サーバーに対して以下のRCONコマンドを実行する。
  ```
  say <ユーザー名>: <メッセージ本文>
  ```

### 5.3 `/pong`コマンド

- bot自身の説明文を返信するのみ。サーバー状態には関与しない。

### 5.4 `/list`コマンド

- コマンドが実行されたチャンネルに対応する`ServerConfig`を検索する。
  - 見つからない場合:「このチャンネルに紐づいたサーバーが見つかりません」と返信。
- 見つかった場合、対象サーバーに対して RCON `list` コマンドを実行し、結果(オンラインプレイヤー一覧)をそのまま返信する。
- RCON実行が失敗した場合はエラー内容を含めて返信する(パニックしない)。

### 5.5 `/status`コマンド

- 設定されている全サーバーに対して、RCON `list` コマンドを5秒のタイムアウト付きで実行する。
- 各サーバーについて以下のいずれかの行を生成し、まとめて1つのメッセージとして返信する。
  - 成功: `🟢 <サーバー名>: 起動中 (<listコマンドの出力>)`
  - タイムアウトまたは失敗: `🔴 <サーバー名>: 応答なし`

### 5.6 `/stop`コマンド

- コマンドが実行されたチャンネルに対応する`ServerConfig`を検索する。
  - 見つからない場合:「このチャンネルに紐づいたサーバーが見つかりません」と返信。
- 見つかった場合、対象サーバーに対して RCON `stop` コマンドを実行し、サーバーを停止する。成功時は「<サーバー名>を停止しました」と返信する。
- RCON実行が失敗した場合はエラー内容を含めて返信する(パニックしない)。
- **権限制限なし**: 実行者を限定していない。信頼できるメンバーのみが参加するサーバー運用を前提とした意図的な仕様。不特定多数が参加するサーバーで運用する場合は、`poise`の権限チェック(`required_permissions`等)を追加すること。

## 6. RCON実行(`rcon::run`)

```rust
async fn run(server: &ServerConfig, command: &str) -> Result<String, Error>
```

- `mcrcon -H <host> -P <port> -p <password> "<command>"` を`tokio::process::Command`で非同期実行する(bot本体のイベントループをブロックしない)。
- 標準出力をUTF-8として読み取り、前後の空白を除去したうえで、mcrconが付与するANSIエスケープシーケンス(色・リセットコード等)を正規表現で除去して返す。
- プロセスの終了コードが非ゼロの場合はエラーとして返す(標準エラー出力の内容を含む)。

## 7. デプロイ

- `Dockerfile`: `rust:latest`イメージ上で`mcrcon`をソースからビルド・インストールし、`cargo build --release`でbotをビルドする。
- `docker-compose.yml`(リポジトリルート): `mc_dis_bot_meetwo`サービスとしてビルドし、ホストのMinecraftサーバーログディレクトリをread-onlyでマウントする。RCON通信は`host.docker.internal`経由でホスト上のMinecraftサーバーに到達する想定。

## 8. 既知の制限・TODO

- `.env`の`SERVER_{n}_CHANNEL_ID`等は起動時に一括で検証されるため、1つでも未設定だとbot全体が起動しない。設定ミスに気づきやすい反面、部分的な起動はできない。
- `mcrcon`の起動オーバーヘッド: `/list`・`/status`・チャット中継のたびに`mcrcon`プロセスを都度起動している。呼び出し頻度が高い場合はRCON接続を張りっぱなしにする方式への変更を検討余地あり。
- ログファイルの監視は1秒間隔のポーリング。よりリアルタイム性が必要な場合は`notify`クレート等によるファイルシステムイベント監視への切り替えを検討。
- 任意のRCONコマンドをDiscordから実行できる管理者向けコマンド(例: `/rcon <server> <command>`)は未実装。
