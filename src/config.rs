use anyhow::Context;
use cqhttp_bot_frame::bot::BotConfig;
use serde::Deserialize;
use std::path::Path;
use tencentcloud_sdk::config::ClientConfig;

#[derive(Debug, Deserialize)]
pub struct PsmConfig {
    pub csp: CSPConfig,
    pub bot: Option<BotConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CSPConfig {
    TencentCloud(ClientConfig),
}

pub fn load_from_file(path: &Path) -> anyhow::Result<PsmConfig> {
    config::Config::builder()
        .add_source(config::File::from(path))
        .build()
        .with_context(|| format!("failed to load configuration from {}", path.display()))?
        .try_deserialize()
        .context("failed to deserialize configuration")
}

pub fn default_config() -> String {
    r#"csp:
    tencent_cloud:
        ak: ak
        sk: sk
bot:
    websocket: ws://127.0.0.1:9002/ws
    bot_qq: 123
    root_qq: 345
"#
    .into()
}
