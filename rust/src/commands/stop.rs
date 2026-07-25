use crate::{rcon, Context, Error};

#[poise::command(slash_command)]
pub async fn stop(ctx: Context<'_>) -> Result<(), Error> {
    let Some(server) = ctx
        .data()
        .servers
        .iter()
        .find(|s| s.channel_id == ctx.channel_id())
    else {
        ctx.say("このチャンネルに紐づいたサーバーが見つかりません").await?;
        return Ok(());
    };

    match rcon::run(server, "stop").await {
        Ok(_) => ctx.say(format!("{}を停止しました", server.name)).await?,
        Err(e) => ctx.say(format!("だめでした: {e}")).await?,
    };
    Ok(())
}
