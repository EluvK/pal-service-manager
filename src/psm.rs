use std::sync::Arc;

use async_trait::async_trait;
use clap::Parser;
use cqhttp_bot_frame::{
    bot::{Bot, Handler},
    RecvMsg, SendMsg,
};
use tencentcloud_sdk::client::TencentCloudClient;
use tokio::sync::mpsc::Sender;

use crate::config::{PsmConfig, ServerConfig};

pub struct PalServiceManager {
    _bot_send_tx: Arc<Sender<SendMsg>>, // might be useless
}

impl PalServiceManager {
    pub async fn new(config: PsmConfig) -> Self {
        // need ref
        let csp_config = match config.server {
            ServerConfig::TencentCloud(tencent_cloud_config) => tencent_cloud_config,
        };

        let client = Arc::new(TencentCloudClient::new(&csp_config));

        let (instant_tx, instant_rx) = tokio::sync::mpsc::channel::<SendMsg>(10);
        let bot_send_tx = Arc::new(instant_tx); // maybe useless...
        let task_handler = Arc::new(PalTaskHandler::new(client, bot_send_tx.clone()));

        if let Some(bot_config) = config.bot {
            let bot = Bot::new(bot_config, task_handler, instant_rx).await;
            tokio::spawn(async move {
                bot.start().await;
            });
        }

        Self {
            _bot_send_tx: bot_send_tx,
        }
    }
}

struct PalTaskHandler {
    client: Arc<TencentCloudClient>, // todo ref to Arc<dyn CSP>
    bot_instant_tx: Arc<Sender<SendMsg>>,
}

impl PalTaskHandler {
    pub fn new(client: Arc<TencentCloudClient>, bot_instant_tx: Arc<Sender<SendMsg>>) -> Self {
        Self {
            client,
            bot_instant_tx,
        }
    }
}

#[derive(Debug, Parser)]
struct BotCmd {
    #[arg(short, long)]
    name: String,
}

#[async_trait]
impl Handler for PalTaskHandler {
    type Cmd = BotCmd;
    async fn handle_msg(&self, msg: RecvMsg) -> Option<SendMsg> {
        None
    }
    async fn handle_cmd(&self, cmd: Self::Cmd) -> Option<SendMsg> {
        None
    }
    fn check_cmd_auth(&self, cmd: &Self::Cmd, ori_msg: &RecvMsg) -> bool {
        true
    }
}
