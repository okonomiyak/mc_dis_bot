mod commands;
mod rcon;
mod server_config;

use std::sync::Arc;

use poise::serenity_prelude::{self as serenity};
use regex::Regex;

use server_config::ServerConfig;

pub struct Data {
    servers: Arc<Vec<ServerConfig>>,
}
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;

use commands::{list, pong, status, stop};

type LogPatterns = Vec<(Regex, Box<dyn Fn(&str) -> String + Send + Sync>)>;

/// ログファイルの追記分だけを監視してDiscordに転送する
async fn write_discord(
    server: ServerConfig,
    patterns: Arc<LogPatterns>,
    http: Arc<serenity::Http>,
) {
    tokio::spawn(async move {
        use std::io::{Read, Seek, SeekFrom};

        let path = server.log_path;
        let channel_id = server.channel_id;

        let mut offset: u64 = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let mut partial_line = String::new();

        loop {
            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;

            let mut file = match std::fs::File::open(&path) {
                Ok(f) => f,
                Err(_) => continue, // 読めなかったら今回はスキップして次のループへ
            };

            let len = match file.metadata() {
                Ok(m) => m.len(),
                Err(_) => continue,
            };

            if len < offset {
                // ログローテーション等でファイルが縮小した場合は最初から読み直す
                offset = 0;
                partial_line.clear();
            }

            if len == offset {
                continue; // 追記なし
            }

            if file.seek(SeekFrom::Start(offset)).is_err() {
                continue;
            }

            let mut buf = String::new();
            if file.read_to_string(&mut buf).is_err() {
                continue;
            }
            offset = len;

            partial_line.push_str(&buf);
            let ends_with_newline = partial_line.ends_with('\n');
            let mut lines: Vec<String> = partial_line.lines().map(String::from).collect();
            // 改行で終わっていない最後の行は、次の書き込みで完成するまで持ち越す
            partial_line = if ends_with_newline {
                String::new()
            } else {
                lines.pop().unwrap_or_default()
            };

            for message in &lines {
                let result = patterns.iter().find_map(|(re, formatter)| {
                    re.captures(message).map(|caps| {
                        let captured = caps.get(1).map_or("", |m| m.as_str());
                        formatter(captured)
                    })
                });

                if let Some(text) = result
                    && let Err(e) = channel_id.say(&http, text).await
                {
                    eprintln!("ERROR: Discordへの送信に失敗しました: {e}");
                }
            }
        }
    });
}

async fn event_handler(
    _ctx: &serenity::Context,
    event: &serenity::FullEvent,
    _framework: poise::FrameworkContext<'_, Data, Error>,
    data: &Data,
) -> Result<(), Error> {
    if let serenity::FullEvent::Message { new_message } = event {
        if new_message.author.bot {
            return Ok(());
        }

        let Some(server) = data
            .servers
            .iter()
            .find(|s| s.channel_id == new_message.channel_id)
        else {
            return Ok(());
        };

        let user = new_message.author.name.clone();
        let message = format!("say {user}: {}", new_message.content);
        eprintln!("DEBUG: sending to {} -> {:?}", server.name, message);

        if let Err(e) = rcon::run(server, &message).await {
            eprintln!("ERROR: rcon送信に失敗しました({}): {e}", server.name);
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    dotenvy::from_filename(".env").ok();

    let token = std::env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN");

    let intents = serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;

    let servers = ServerConfig::load_all_from_env().expect("failed to load server config from env");

    let re_chat = Regex::new(r"MinecraftServer/\]: (<.+>.+|.+ (?:joined|left) the game)").unwrap();
    let re_rcon = Regex::new(r"\[Not Secure\] \[Rcon\] (.+)").unwrap();
    let re_start = Regex::new(r"Dedicated server took ([\d.]+) seconds to load").unwrap();
    let re_stop = Regex::new(r"Stopping (?:the )?server").unwrap();
    let re_death =
        Regex::new(r"MinecraftServer/\]: (.+ (?:died|was slain|fell|drowned|burned|blew up).*)").unwrap();
    let re_advancement = Regex::new(
        r"MinecraftServer/\]: (.+ has (?:made the advancement|reached the goal|completed the challenge) \[.+\])",
    )
    .unwrap();

    let patterns: LogPatterns = vec![
        (re_rcon, Box::new(|cap: &str| cap.to_string())),
        (re_start, Box::new(|cap: &str| format!("サーバー起動完了({cap}秒)"))),
        (re_stop, Box::new(|_| "サーバー終了".to_string())),
        (re_advancement, Box::new(|cap: &str| format!("実績解除:{cap}"))),
        (re_death, Box::new(|cap: &str| cap.to_string())),
        (
            re_chat,
            Box::new(|cap: &str| {
                // チャットメッセージ `<user> msg` は `user`:msg 形式に変換する。
                if let Some(rest) = cap.strip_prefix('<')
                    && let Some(idx) = rest.find('>')
                {
                    let user = &rest[..idx];
                    let message = rest[idx + 1..].trim_start();
                    return format!("`{user}`:{message}");
                }
                // 参加/退出通知は `user`が入室/退出しました。 に変換する。
                if let Some(user) = cap.strip_suffix(" joined the game") {
                    return format!("`{user}`が入室しました。");
                }
                if let Some(user) = cap.strip_suffix(" left the game") {
                    return format!("`{user}`が退出しました。");
                }
                cap.to_string()
            }),
        ),
    ];
    let patterns = Arc::new(patterns);

    let servers_for_setup = servers.clone();
    let servers = Arc::new(servers);

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![pong(), list(), status(), stop()],
            event_handler: |ctx, event, framework, data| Box::pin(event_handler(ctx, event, framework, data)),
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {
            let http = ctx.http.clone();

            Box::pin(async move {
                for server in servers_for_setup {
                    write_discord(server, patterns.clone(), http.clone()).await;
                }
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                Ok(Data { servers })
            })
        })
        .build();

    let client = serenity::ClientBuilder::new(token, intents).framework(framework).await;
    client.unwrap().start().await.unwrap();
}
