use cqhttp_bot_frame::bot::BotConfig;
use serde::Deserialize;
use std::{fs::File, io::Read};
use tencentcloud_sdk::config::ClientConfig;

#[derive(Debug, Deserialize)]
pub struct PsmConfig {
    pub server: ServerConfig,
    pub bot: Option<BotConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum ServerConfig {
    TencentCloud(ClientConfig),
}

pub fn load_from_file() -> PsmConfig {
    let mut file = File::open("config.yaml").expect("Failed to open config file");
    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .expect("Failed to read config file");
    serde_yaml::from_str(&contents).expect("Failed to parse config file")
}
