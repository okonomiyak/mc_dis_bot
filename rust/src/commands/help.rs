use crate::{Context, Error};

#[poise::command(slash_command, prefix_command)]
pub async fn pong(
    ctx: Context<'_>,
) -> Result<(), Error> {
    ctx.say("MinecraftサーバーのログをDiscordに転送し、コマンドでサーバーに指示を送れるbot").await?;
    Ok(())
}