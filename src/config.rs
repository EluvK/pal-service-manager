use anyhow::Context;
use cqhttp_bot_frame::bot::BotConfig;
use serde::Deserialize;
use std::path::Path;
use tencentcloud_sdk::config::ClientConfig;

use crate::local_storage::LocalSaveStorageConfig;

#[derive(Debug, Deserialize, Clone)]
pub struct PsmConfig {
    pub csp: CSPConfig,
    pub bot: Option<BotConfig>,
    pub storage: SaveStorageConfig,
    pub ssh: SshConfig,
    pub nps: NpsAccessConfig,
    pub whitelist: WhiteListConfig,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum CSPConfig {
    TencentCloud(ClientConfig),
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum SaveStorageConfig {
    Local(LocalSaveStorageConfig),
}

#[derive(Debug, Deserialize, Clone)]
pub struct SshConfig {
    pub prikey: String,
    pub user: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct NpsAccessConfig {
    pub region: String,
    pub instance_id: String,
    pub protocol: String,
    pub port: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WhiteListConfig {
    pub server: Vec<u64>,
    pub nps: Vec<u64>,
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
storage:
    local:
    local_dir: /home/ubuntu/psm
    remote_dir: /home/ubuntu/psm
ssh:
    prikey: /home/ubuntu/.ssh/id_ed25519
    user: ubuntu
nps:
    region: ap-shanghai
    instance_id: ins-123
    protocol: tcp
    port: 80
whitelist:
    server: [123, 456]
    nps: [123, 456]
"#
    .into()
}
