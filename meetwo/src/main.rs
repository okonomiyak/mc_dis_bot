use poise::serenity_prelude::{self as serenity};
use regex::Regex;
use std::process::Command;
struct Data {
    channel_id: serenity::ChannelId,
}
type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, Data, Error>;


/// Displays your or another user's account creation date
#[poise::command(slash_command, prefix_command)]
async fn pong(
    ctx: Context<'_>,
) -> Result<(), Error> {
    ctx.say("MinecraftサーバーのログをDiscordに転送し、コマンドでサーバーに指示を送れるbot").await?;
    Ok(())
}
#[poise::command(slash_command)]
async fn say(
    ctx: Context<'_>,
    #[description = "メッセージ"] text: String,   
)-> Result<(),Error> {
    match std::env::current_dir() { // カレントディレクトリを取得
        Ok(x) => 
            {
                Command::new("./user.sh")
                    .arg(&text)
                    .current_dir(x)
                    .output()
                    .expect("failed to execute process");
                ctx.say("OK").await?;
            },
        Err(_) => {
            ctx.say("だめでした").await?;
        },
    }
    Ok(())
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

    let re = Regex::new(r"MinecraftServer/\]: (<.+>.+|.+ (?:joined|left) the game|Stopping (?:the )?server)").unwrap();
    let re1 = Regex::new(r"\[Not Secure\] \[Rcon\] (.+)").unwrap();
    let re2 = Regex::new(r"Dedicated server took ([\d.]+) seconds to load").unwrap();

    let framework = poise::Framework::builder()
        .options(poise::FrameworkOptions {
            commands: vec![pong()],
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
                    eprintln!("DEBUG: lines={}, log_line={}", lines.len(), log_line); // ← 追加

                    if lines.len() < log_line {
                        log_line = 0;
                    }

                    for message in &lines[log_line..] {
                        if let Some(caps) = re1.captures(&message) {
                            channel_id.say(&http, &caps[1]).await.unwrap();
                        }
                        else if let Some(caps) = re.captures(&message) {
                            channel_id.say(&http, &caps[1]).await.unwrap();
                        }
                        else if let Some(caps) = re2.captures(&message) {
                            channel_id.say(&http, format!("サーバー起動完了({}秒)", &caps[1])).await.unwrap();
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