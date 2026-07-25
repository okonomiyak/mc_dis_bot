use poise::serenity_prelude as serenity;

use crate::Error;

#[derive(Clone)]
pub struct ServerConfig {
    pub name: String,
    pub channel_id: serenity::ChannelId,
    pub log_path: String,
    pub rcon_host: String,
    pub rcon_port: u16,
    pub rcon_password: String,
}

impl ServerConfig {
    /// `SERVER_COUNT` と `SERVER_{n}_*` の環境変数からサーバー設定一覧を読み込む
    pub fn load_all_from_env() -> Result<Vec<Self>, Error> {
        let count: usize = std::env::var("SERVER_COUNT")
            .map_err(|_| "missing SERVER_COUNT")?
            .parse()
            .map_err(|_| "invalid SERVER_COUNT")?;

        (1..=count).map(Self::from_env).collect()
    }

    fn from_env(i: usize) -> Result<Self, Error> {
        let get = |key: &str| -> Result<String, Error> {
            let var = format!("SERVER_{i}_{key}");
            std::env::var(&var).map_err(|_| format!("missing {var}").into())
        };

        Ok(ServerConfig {
            name: get("NAME")?,
            channel_id: serenity::ChannelId::new(get("CHANNEL_ID")?.parse()?),
            log_path: get("LOG_PATH")?,
            rcon_host: get("RCON_HOST")?,
            rcon_port: get("RCON_PORT")?.parse()?,
            rcon_password: get("RCON_PASSWORD")?,
        })
    }
}
