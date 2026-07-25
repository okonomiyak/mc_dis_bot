use std::sync::LazyLock;
use std::time::SystemTime;

use poise::serenity_prelude as serenity;
use regex::Regex;

use crate::{rcon, Context, Error};

static PLAYER_LIST_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"There are (\d+) of a max of (\d+) players online:?\s*(.*)").unwrap());

const COLOR_ONLINE: (u8, u8, u8) = (46, 204, 113);
const COLOR_OFFLINE: (u8, u8, u8) = (231, 76, 60);

/// ログファイルの最終更新時刻を「n秒前」のような表記にする
fn log_freshness(log_path: &str) -> String {
    let Ok(modified) = std::fs::metadata(log_path).and_then(|m| m.modified()) else {
        return "不明".to_string();
    };
    let Ok(elapsed) = SystemTime::now().duration_since(modified) else {
        return "不明".to_string();
    };

    let secs = elapsed.as_secs();
    if secs < 60 {
        format!("{secs}秒前")
    } else if secs < 3600 {
        format!("{}分前", secs / 60)
    } else if secs < 86400 {
        format!("{}時間前", secs / 3600)
    } else {
        format!("{}日前", secs / 86400)
    }
}

/// `list`コマンドの出力から「オンライン人数/最大人数」と参加者名を抽出する
fn parse_player_list(output: &str) -> (String, Option<String>) {
    match PLAYER_LIST_RE.captures(output) {
        Some(caps) => {
            let online = &caps[1];
            let max = &caps[2];
            let names = caps[3].trim();
            let names = (!names.is_empty()).then(|| names.to_string());
            (format!("{online}/{max}人"), names)
        }
        None => (output.to_string(), None),
    }
}

#[poise::command(slash_command)]
pub async fn status(ctx: Context<'_>) -> Result<(), Error> {
    let mut reply = poise::CreateReply::default();

    for server in ctx.data().servers.iter() {
        let result = tokio::time::timeout(std::time::Duration::from_secs(5), rcon::run(server, "list")).await;

        let log_age = log_freshness(&server.log_path);

        let embed = match result {
            Ok(Ok(output)) => {
                let (count, names) = parse_player_list(&output);
                let embed = serenity::CreateEmbed::new()
                    .title(format!("🟢 {}", server.name))
                    .color(COLOR_ONLINE)
                    .field("プレイヤー", count, true)
                    .field("最終ログ更新", log_age, true);
                match names {
                    Some(names) => embed.field("参加者", names, false),
                    None => embed,
                }
            }
            _ => serenity::CreateEmbed::new()
                .title(format!("🔴 {}", server.name))
                .color(COLOR_OFFLINE)
                .field("状態", "応答なし", true)
                .field("最終ログ更新", log_age, true),
        };

        reply = reply.embed(embed);
    }

    ctx.send(reply).await?;
    Ok(())
}
