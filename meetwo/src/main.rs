mod commands;

use poise::serenity_prelude::{self as serenity};
use regex::Regex;

pub struct Data {
    channel_id: serenity::ChannelId,
}
pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;

use commands::{list, pong};

/// Displays your or another user's account creation date



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
        if new_message.channel_id == data.channel_id {
            let user = new_message.author.name.clone();
            let message = format!("{user}: {}", new_message.content);
            eprintln!("DEBUG: passing to user.sh -> {:?}", message); 

            let current_dir = std::env::current_dir()?;
            std::process::Command::new("./user.sh")
                .arg(message)
                .current_dir(current_dir)
                .output()?;
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() {
    dotenvy::from_filename(".env").ok();
    let token = std::env::var("DISCORD_TOKEN").expect("missing DISCORD_TOKEN");

    let path = std::env::var("LOG_PATH").expect("missing LOG_PATH");

    let intents = serenity::GatewayIntents::non_privileged()
    | serenity::GatewayIntents::MESSAGE_CONTENT;

    let channel_id_str = std::env::var("CHANNEL_ID").expect("missing CHANNEL_ID");
    let channel_id = serenity::ChannelId::new(channel_id_str.parse().expect("invalid CHANNEL_ID"));

    let re_chat   = Regex::new(r"MinecraftServer/\]: (<.+>.+|.+ (?:joined|left) the game)").unwrap();
    let re_rcon   = Regex::new(r"\[Not Secure\] \[Rcon\] (.+)").unwrap();
    let re_start  = Regex::new(r"Dedicated server took ([\d.]+) seconds to load").unwrap();
    let re_stop   = Regex::new(r"Stopping (?:the )?server").unwrap();
    let re_death  = Regex::new(r"MinecraftServer/\]: (.+ (?:died|was slain|fell|drowned|burned|blew up).*)").unwrap();
    let re_advancement = Regex::new(r"MinecraftServer/\]: (.+ has (?:made the advancement|reached the goal|completed the challenge) \[.+\])").unwrap();

    let patterns: Vec<(Regex, Box<dyn Fn(&str) -> String + Send + Sync>)> = vec![
        (re_rcon,  Box::new(|cap: &str| cap.to_string())),
        (re_start, Box::new(|cap: &str| format!("サーバー起動完了({cap}秒)"))),
        (re_stop,  Box::new(|_| "サーバー終了".to_string())),
        (re_advancement, Box::new(|cap: &str| format!("実績解除:{cap}"))),
        (re_death, Box::new(|cap: &str| cap.to_string())),
        (re_chat,  Box::new(|cap: &str| cap.to_string())),
    ];

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![pong(), list()],
            event_handler: |ctx, event, framework, data| {
                Box::pin(event_handler(ctx, event, framework, data))
            },
            ..Default::default()
        })
        .setup(move |ctx, _ready, framework| {

            let http = ctx.http.clone();
            tokio::spawn(async move{
                let mut log_line = std::fs::read_to_string(&path).expect("NO file").lines().count();
                loop{
                    let contents = match std::fs::read_to_string(&path) {
                        Ok(c) => c,
                        Err(_) => {
                            tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                            continue; // 読めなかったら今回はスキップして次のループへ
                        }
                    };
                    let lines :Vec<&str> = contents.lines().collect();

                    if lines.len() < log_line {
                        log_line = 0;
                    }
                    for message in &lines[log_line..] {
                        let result = patterns.iter().find_map(|(re, formatter)| {
                            re.captures(message).map(|caps| {
                                let captured = caps.get(1).map_or("", |m| m.as_str());
                                formatter(captured)
                            })
                        });

                        if let Some(text) = result {
                            channel_id.say(&http, text).await.unwrap();
                        }
                    }
                    log_line = lines.len();
                    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                }
            });

            Box::pin(async move {
                poise::builtins::register_globally(ctx, &framework.options().commands).await?;
                //Poise::builtins::register_in_guild(
                //    ctx,
                //    &framework.options().commands,
                //    serenity::GuildId::new(SERVER),
                //).await?;
                Ok(Data { channel_id })
            })
        })
        .build();

    let client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
        .await;
    client.unwrap().start().await.unwrap();
}