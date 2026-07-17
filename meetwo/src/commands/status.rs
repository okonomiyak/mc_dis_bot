use std::time::SystemTime;

use crate::{rcon, Context, Error};

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

#[poise::command(slash_command)]
pub async fn status(ctx: Context<'_>) -> Result<(), Error> {
    let mut lines = Vec::new();

    for server in ctx.data().servers.iter() {
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            rcon::run(server, "list"),
        )
        .await;

        let log_age = log_freshness(&server.log_path);

        let line = match result {
            Ok(Ok(output)) => format!(
                "🟢 **{}**\n　プレイヤー: {output}\n　最終ログ更新: {log_age}",
                server.name
            ),
            _ => format!(
                "🔴 **{}**: 応答なし\n　最終ログ更新: {log_age}",
                server.name
            ),
        };
        lines.push(line);
    }

    ctx.say(lines.join("\n\n")).await?;
    Ok(())
}
