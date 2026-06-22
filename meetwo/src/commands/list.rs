use crate::{Context, Error};
use std::process::Command;

#[poise::command(slash_command)]
pub async fn list(
    ctx: Context<'_>,
)-> Result<(),Error> {
    match std::env::current_dir() { // カレントディレクトリを取得
        Ok(x) => 
            {
                Command::new("./user.sh")
                    .arg("list")
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
