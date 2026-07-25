use std::sync::LazyLock;

use regex::Regex;

use crate::server_config::ServerConfig;
use crate::Error;

static ANSI_ESCAPE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap());

/// 指定したサーバーに対して mcrcon で任意のコマンドを実行し、標準出力を返す
pub async fn run(server: &ServerConfig, command: &str) -> Result<String, Error> {
    let output = tokio::process::Command::new("mcrcon")
        .arg("-H")
        .arg(&server.rcon_host)
        .arg("-P")
        .arg(server.rcon_port.to_string())
        .arg("-p")
        .arg(&server.rcon_password)
        .arg(command)
        .output()
        .await?;

    if !output.status.success() {
        return Err(format!(
            "mcrcon exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    Ok(ANSI_ESCAPE.replace_all(raw.trim(), "").to_string())
}
