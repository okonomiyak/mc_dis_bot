use crate::{rcon, Context, Error};

#[poise::command(slash_command)]
pub async fn status(ctx: Context<'_>) -> Result<(), Error> {
    let mut lines = Vec::new();

    for server in ctx.data().servers.iter() {
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            rcon::run(server, "list"),
        )
        .await;

        let line = match result {
            Ok(Ok(output)) => format!("🟢 {}: 起動中 ({output})", server.name),
            _ => format!("🔴 {}: 応答なし", server.name),
        };
        lines.push(line);
    }

    ctx.say(lines.join("\n")).await?;
    Ok(())
}
