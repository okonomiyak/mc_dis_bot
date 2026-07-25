use crate::{rcon, Context, Error};

#[poise::command(slash_command)]
pub async fn list(ctx: Context<'_>) -> Result<(), Error> {
    let Some(server) = ctx
        .data()
        .servers
        .iter()
        .find(|s| s.channel_id == ctx.channel_id())
    else {
        ctx.say("このチャンネルに紐づいたサーバーが見つかりません").await?;
        return Ok(());
    };

    match rcon::run(server, "list").await {
        Ok(output) => ctx.say(output).await?,
        Err(e) => ctx.say(format!("だめでした: {e}")).await?,
    };
    Ok(())
}
