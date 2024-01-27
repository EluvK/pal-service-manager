use std::{fmt::Display, str::FromStr, sync::Arc, time::Duration};

use async_trait::async_trait;
use cqhttp_bot_frame::{
    bot::{Bot, Handler},
    RecvMsg, SendMsg,
};
use tencentcloud_sdk::{client::TencentCloudClient, constant::Region};
use tokio::sync::{mpsc::Sender, Mutex};
use tracing::{debug, error, info};

use crate::{
    bot_cmd::Commands,
    config::{CSPConfig, PsmConfig},
    constant::ServiceInstanceType,
    cvm_utils::{query_cvm_ip, query_key_ids, query_spot_paid_price},
    server_status::{ServerManager, Status},
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
        let server_status_manager = Arc::new(Mutex::new(ServerManager::new(server_status_path)));

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
    server_status_manager: Arc<Mutex<ServerManager>>,
}

impl PalTaskHandler {
    pub fn new(
        client: Arc<TencentCloudClient>,
        bot_instant_tx: Arc<Sender<SendMsg>>,
        server_status_manager: Arc<Mutex<ServerManager>>,
    ) -> Self {
        Self {
            client,
            bot_instant_tx,
            server_status_manager,
        }
    }
    fn err_log(e: impl Display) {
        error!("PalTaskHandler ERROR :{e}");
    }
    async fn reply_err_msg(&self, content: String, msg: &RecvMsg) {
        self.bot_instant_tx
            .send(msg.reply(content))
            .await
            .unwrap_or_else(Self::err_log);
    }
    async fn list_server(&self, server: String, msg: &RecvMsg) {
        let content = match self.server_status_manager.lock().await.list(&server) {
            Ok(result) => result,
            Err(msg) => msg,
        };
        self.bot_instant_tx
            .send(msg.reply(content))
            .await
            .unwrap_or_else(Self::err_log);
    }
    async fn start_server(&self, server: String, msg: &RecvMsg) -> Result<(), String> {
        self.server_status_manager
            .lock()
            .await
            .check_server_status(&server, &Status::Stopped)?;
        self.server_status_manager
            .lock()
            .await
            .create_server(&server)?;

        let candidate_regions = &[Region::Guangzhou, Region::Nanjing, Region::Shanghai];
        let instance_type: ServiceInstanceType = self
            .server_status_manager
            .lock()
            .await
            .get_instance_type(&server)?
            .try_into()?;
        let (price, (region, zone, instance_type)) =
            query_spot_paid_price(&self.client, candidate_regions, instance_type)
                .await
                .map_err(|e| format!("query spot paid price err: {e}"))?;
        self.bot_instant_tx
            .send(msg.reply(format!(
                "Finding lowest price server {} in {} at {:?}",
                instance_type, region, price
            )))
            .await
            .unwrap_or_else(Self::err_log);
        let key_ids = query_key_ids(&self.client)
            .await
            .map_err(|e| format!("query key err: {e}"))?;
        let server_id = self
            .client
            .cvm()
            .instances()
            .run_instance(
                &region,
                &zone,
                &instance_type,
                key_ids,
                vec!["sg-o25f29eq".into()],
            ) // todo sg
            .await
            .map_err(|e| format!("init server err: {e}"))?;
        self.bot_instant_tx
            .send(msg.reply(format!("Success init server, id: {:?}", server_id)))
            .await
            .unwrap_or_else(Self::err_log);
        let ip = query_cvm_ip(&self.client, &region, &server_id)
            .await
            .map_err(|e| format!("get cvm ip failed :{e}"))?;
        // todo add build server script exec
        self.bot_instant_tx
            .send(msg.reply(format!("Success create server, ip-port: {:?}:8211", ip)))
            .await
            .unwrap_or_else(Self::err_log);
        self.server_status_manager
            .lock()
            .await
            .finish_creating_server(
                &server,
                &format!("{}:8211", ip),
                &region.to_string(),
                &server_id,
            )?;

        Ok(())
    }
    async fn stop_server(&self, server: String, msg: &RecvMsg) -> Result<(), String> {
        self.server_status_manager
            .lock()
            .await
            .check_server_status(&server, &Status::Running)?;
        // todo add bk files maybe?
        let (region, instance_id) = self
            .server_status_manager
            .lock()
            .await
            .stop_server(&server)?;
        let region = Region::from_str(&region).unwrap();
        self.client
            .cvm()
            .instances()
            .terminate_instance(&region, &instance_id)
            .await
            .map_err(|e| format!("delete server err: {e}"))?;
        self.bot_instant_tx
            .send(msg.reply(format!(
                "Success delete server {server} instance id: {instance_id}",
            )))
            .await
            .unwrap_or_else(Self::err_log);
        self.server_status_manager
            .lock()
            .await
            .finish_stopping_server(&server)?;

        Ok(())
    }

    pub async fn handle_server_cmd(
        &self,
        status: Option<String>,
        start: Option<String>,
        stop: Option<String>,
        msg: &RecvMsg,
    ) -> Option<SendMsg> {
        if let Some(server) = status {
            self.list_server(server, msg).await;
        }
        if let Some(server) = start {
            if let Err(content) = self.start_server(server, msg).await {
                self.reply_err_msg(content, msg).await;
            }
        }
        if let Some(server) = stop {
            if let Err(content) = self.stop_server(server, msg).await {
                self.reply_err_msg(content, msg).await;
            }
        }
        Some(msg.reply("cmd exec finish.".into()))
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
                Commands::Config { r#type: _type } => None,
            };
            return res;
        }
        None
    }
    async fn check_cmd_auth(&self, cmd: &Self::Cmd, ori_msg: &RecvMsg, root_id: u64) -> bool {
        let root_cmd = cmd.sub.as_ref().is_some_and(|c| {
            if let Commands::Server {
                status: _status,
                start,
                stop,
            } = c
            {
                start.is_some() || stop.is_some()
            } else {
                false
            }
        });
        debug!("is root cmd: {root_cmd}");
        !root_cmd || ori_msg.from_id == root_id
    }
}
