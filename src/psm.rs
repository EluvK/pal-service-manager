use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use cqhttp_bot_frame::{
    bot::{Bot, Handler},
    RecvMsg, SendMsg,
};
use tencentcloud_sdk::client::TencentCloudClient;
use tokio::sync::mpsc::Sender;
use tracing::{error, info};

use crate::{
    bot_cmd::Commands,
    config::{CSPConfig, PsmConfig},
    server_status::ServerManager,
};

pub struct PalServiceManager {
    _bot_send_tx: Arc<Sender<SendMsg>>, // might be useless
}

impl PalServiceManager {
    pub async fn new(config: PsmConfig, server_status_path: &std::path::Path) -> Self {
        // need ref
        let csp_config = match config.csp {
            CSPConfig::TencentCloud(tencent_cloud_config) => tencent_cloud_config,
        };

        let client = Arc::new(TencentCloudClient::new(&csp_config));
        let server_status_manager = Arc::new(ServerManager::new(server_status_path));

        let (instant_tx, instant_rx) = tokio::sync::mpsc::channel::<SendMsg>(10);
        let bot_send_tx = Arc::new(instant_tx); // maybe useless...
        let task_handler = Arc::new(PalTaskHandler::new(
            client,
            bot_send_tx.clone(),
            server_status_manager,
        ));

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

    pub async fn start(&self) -> ! {
        loop {
            tokio::time::sleep(Duration::from_secs(10)).await;
        }
    }
}

struct PalTaskHandler {
    client: Arc<TencentCloudClient>, // todo ref to Arc<dyn CSP>
    bot_instant_tx: Arc<Sender<SendMsg>>,
    server_status_manager: Arc<ServerManager>,
}

impl PalTaskHandler {
    pub fn new(
        client: Arc<TencentCloudClient>,
        bot_instant_tx: Arc<Sender<SendMsg>>,
        server_status_manager: Arc<ServerManager>,
    ) -> Self {
        Self {
            client,
            bot_instant_tx,
            server_status_manager,
        }
    }
    async fn list_server(&self, server: Option<String>, msg: &RecvMsg) {
        if let Err(e) = self
            .bot_instant_tx
            .send(msg.reply(self.server_status_manager.list(server)))
            .await
        {
            error!("instant msg send err: {e}")
        }
    }

    pub async fn handle_server_cmd(
        &self,
        status: Option<String>,
        start: Option<String>,
        stop: Option<String>,
        msg: &RecvMsg,
    ) -> Option<SendMsg> {
        self.list_server(status, msg).await;

        if let Some(start) = start {}
        if let Some(stop) = stop {}
        Some(msg.reply("ok".into()))
    }
}

const DEFAULT_REPLY: &str = "使用 `#--help` 来查询命令";

#[async_trait]
impl Handler for PalTaskHandler {
    type Cmd = crate::bot_cmd::BotCmd;
    async fn handle_msg(&self, msg: RecvMsg) -> Option<SendMsg> {
        info!("psm recv msg: {msg:?} ");
        Some(msg.reply(DEFAULT_REPLY.into()))
    }
    async fn handle_cmd(&self, cmd: Self::Cmd, msg: RecvMsg) -> Option<SendMsg> {
        info!("psm recv cmd: {cmd:?}");
        if let Some(cmd) = cmd.sub {
            let res = match cmd {
                Commands::Server {
                    status,
                    start,
                    stop,
                } => self.handle_server_cmd(status, start, stop, &msg).await,
                Commands::Config { r#type } => None,
            };
            return res;
        }
        None
    }
    fn check_cmd_auth(&self, cmd: &Self::Cmd, ori_msg: &RecvMsg) -> bool {
        true
    }
}
