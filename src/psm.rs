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
    config::{CSPConfig, PsmConfig, SaveStorageConfig},
    constant::ServiceInstanceType,
    cvm_utils::{query_cvm_ip, query_key_ids, query_spot_paid_price},
    error::PSMError,
    local_storage::LocalStorage,
    server_status::{ServerManager, Status},
    shell_manager::{Script, ShellManager},
};

pub struct PalServiceManager {
    _bot_send_tx: Arc<Sender<SendMsg>>, // might be useless
}

impl PalServiceManager {
    pub async fn new(config: PsmConfig, server_status_path: &std::path::Path) -> Self {
        // need ref
        let CSPConfig::TencentCloud(csp_config) = config.csp.clone();
        let client = Arc::new(TencentCloudClient::new(&csp_config));

        let server_status_manager = Arc::new(Mutex::new(ServerManager::new(server_status_path)));
        let shell_manager = Arc::new(ShellManager::new(config.ssh.clone()));

        // need_ref
        let SaveStorageConfig::Local(storage_config) = config.storage.clone();
        let local_storage = Arc::new(LocalStorage::new(storage_config));

        let (instant_tx, instant_rx) = tokio::sync::mpsc::channel::<SendMsg>(10);
        let bot_send_tx = Arc::new(instant_tx); // maybe useless...
        let task_handler = Arc::new(PalTaskHandler::new(
            client,
            bot_send_tx.clone(),
            server_status_manager,
            shell_manager,
            local_storage,
            Arc::new(config.clone()),
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
    pub(crate) client: Arc<TencentCloudClient>, // todo ref to Arc<dyn CSP>
    pub(crate) bot_instant_tx: Arc<Sender<SendMsg>>,
    pub(crate) server_status_manager: Arc<Mutex<ServerManager>>,
    pub(crate) shell_manager: Arc<ShellManager>,
    pub(crate) local_storage: Arc<LocalStorage>,
    pub(crate) config: Arc<PsmConfig>,
}

impl PalTaskHandler {
    pub fn new(
        client: Arc<TencentCloudClient>,
        bot_instant_tx: Arc<Sender<SendMsg>>,
        server_status_manager: Arc<Mutex<ServerManager>>,
        shell_manager: Arc<ShellManager>,
        local_storage: Arc<LocalStorage>,
        config: Arc<PsmConfig>,
    ) -> Self {
        Self {
            client,
            bot_instant_tx,
            server_status_manager,
            shell_manager,
            local_storage,
            config,
        }
    }
    fn err_log(e: impl Display) {
        error!("PalTaskHandler ERROR :{e}");
    }
    async fn list_server(&self, server: String, msg: &RecvMsg) {
        let content = match self.server_status_manager.lock().await.list(&server) {
            Ok(result) => result,
            Err(e) => e.to_string(),
        };
        self.bot_instant_tx
            .send(msg.reply(content))
            .await
            .unwrap_or_else(Self::err_log);
    }

    async fn query_and_create_server(
        &self,
        candidate_regions: &[Region],
        instance_type: ServiceInstanceType,
        msg: &RecvMsg,
    ) -> Result<(String, Region, String), String> {
        let (price, (region, zone, instance_type)) =
            query_spot_paid_price(&self.client, candidate_regions, &instance_type)
                .await
                .map_err(|e| format!("query spot paid price err: {e}"))?;
        self.bot_instant_tx
            .send(msg.reply(format!(
                "Finding lowest price server {} in {} with {}/h + {}/GB",
                instance_type,
                region,
                price.instance_price.unit_price_discount,
                price.bandwidth_price.unit_price_discount
            )))
            .await
            .unwrap_or_else(Self::err_log);
        let key_ids = query_key_ids(&self.client)
            .await
            .map_err(|e| format!("query key err: {e}"))?;
        let security_group_id = self
            .client
            .cvm()
            .security_group()
            .describe_security_groups(&region)
            .await
            .map_err(|e| format!("find security group err: {e}"))?
            .into_iter()
            .filter_map(|sg| {
                sg.security_group_name
                    .to_ascii_lowercase()
                    .contains("palworld")
                    .then_some(sg.security_group_id)
            })
            .collect();
        let server_id = self
            .client
            .cvm()
            .instances()
            .run_instance(&region, &zone, &instance_type, key_ids, security_group_id)
            .await
            .map_err(|e| format!("init server err: {e}"))?;
        self.bot_instant_tx
            .send(msg.reply(format!(
                "Success init server, id: {}, install palworld next(will take minutes)",
                server_id
            )))
            .await
            .unwrap_or_else(Self::err_log);
        let ip = query_cvm_ip(&self.client, &region, &server_id)
            .await
            .map_err(|e| format!("get cvm ip failed :{e}"))?;
        Ok((ip, region, server_id))
    }

    async fn start_server(&self, server: &str, msg: &RecvMsg) -> Result<(), PSMError> {
        self.server_status_manager
            .lock()
            .await
            .check_server_status(server, &Status::Stopped)?;
        let (ip, region, server_id) = async {
            self.server_status_manager
                .lock()
                .await
                .create_server(server)?;
            let candidate_regions = vec![Region::Guangzhou, Region::Nanjing, Region::Shanghai];
            let instance_type: ServiceInstanceType = self
                .server_status_manager
                .lock()
                .await
                .get_instance_type(server)?
                .try_into()?;
            let mut try_cnt = 0;
            let (ip, region, server_id) = {
                loop {
                    match self
                        .query_and_create_server(&candidate_regions, instance_type.clone(), msg)
                        .await
                    {
                        Ok(r) => break r,
                        Err(e) => {
                            try_cnt += 1;
                            if try_cnt == 5 {
                                return Err(PSMError::CSPClientError(format!(
                                    "err to create server: {e}"
                                )));
                            }
                        }
                    }
                }
            };
            Ok((ip, region, server_id))
        }
        .await?;
        tokio::time::sleep(Duration::from_secs(10)).await;

        // upload script
        self.local_storage
            .upload_scripts(&self.shell_manager.ssh_config, &ip)
            .await?;

        // add build server script exec
        self.shell_manager.run(&ip, Script::InstallServer).await?;

        let save_name = self
            .server_status_manager
            .lock()
            .await
            .get_save_name(server)?;
        if let Some(save_name) = save_name {
            // sftp bk files
            self.local_storage
                .upload_saves(&save_name, &self.shell_manager.ssh_config, &ip)
                .await?;
            // restore bk saves
            self.shell_manager.run(&ip, Script::RestoreSave).await?;
            self.bot_instant_tx
                .send(msg.reply(format!("Success load save, {}", save_name)))
                .await?;
        }

        // server start
        self.shell_manager.run(&ip, Script::StartServer).await?;
        let ip_port = format!("{}:8211", ip);
        self.bot_instant_tx
            .send(msg.reply(format!("Success create server, ip-port: {ip_port}")))
            .await?;
        self.server_status_manager
            .lock()
            .await
            .finish_creating_server(server, &ip_port, &region.to_string(), &server_id)?;

        Ok(())
    }

    #[inline]
    async fn backup_save(&self, server: &str) -> Result<String, PSMError> {
        // add bk save
        let ip = self
            .server_status_manager
            .lock()
            .await
            .get_server_ip(server)?
            .ok_or(anyhow::anyhow!("failed to get server ip infomation"))?;
        let save_name = self.shell_manager.run(&ip, Script::BackupSave).await?;
        self.local_storage
            .download_saves(&save_name, &self.shell_manager.ssh_config, &ip)
            .await?;
        Ok(save_name)
    }

    async fn stop_server(&self, server: &str, msg: &RecvMsg) -> Result<(), PSMError> {
        self.server_status_manager
            .lock()
            .await
            .check_server_status(server, &Status::Running)?;

        let save_name = self.backup_save(server).await?;

        self.server_status_manager
            .lock()
            .await
            .update_save_name(server, &save_name)?;

        let (region, instance_id) = self
            .server_status_manager
            .lock()
            .await
            .stop_server(server)?;
        let region = Region::from_str(&region).unwrap();
        self.client
            .cvm()
            .instances()
            .terminate_instance(&region, &instance_id)
            .await?;
        self.bot_instant_tx
            .send(msg.reply(format!(
                "Success delete server {server} instance id: {instance_id}",
            )))
            .await
            .unwrap_or_else(Self::err_log);
        self.server_status_manager
            .lock()
            .await
            .finish_stopping_server(server)?;

        Ok(())
    }

    async fn save_server(&self, server: &str, msg: &RecvMsg) -> Result<(), PSMError> {
        self.server_status_manager
            .lock()
            .await
            .check_server_status(server, &Status::Running)?;
        let save_name = self.backup_save(server).await?;
        self.bot_instant_tx
            .send(msg.reply(format!("back save success {save_name}")))
            .await
            .unwrap_or_else(Self::err_log);
        Ok(())
    }

    pub async fn handle_server_cmd(
        &self,
        status: Option<String>,
        start: Option<String>,
        stop: Option<String>,
        save: Option<String>,
        msg: &RecvMsg,
    ) -> Option<SendMsg> {
        if let Some(server) = status {
            self.list_server(server, msg).await;
        }
        if let Some(server) = start {
            if let Err(e) = self.start_server(&server, msg).await {
                self.bot_instant_tx
                    .send(msg.reply(e.to_string()))
                    .await
                    .unwrap_or_else(Self::err_log);
                let res = self
                    .server_status_manager
                    .lock()
                    .await
                    .failed_create_server(&server)
                    .unwrap();
                if let (Some(instance_id), Some(region)) = res {
                    self.client
                        .cvm()
                        .instances()
                        .terminate_instance(&Region::from_str(&region).unwrap(), &instance_id)
                        .await
                        .unwrap();
                }
            }
        }
        if let Some(server) = stop {
            if let Err(e) = self.stop_server(&server, msg).await {
                self.bot_instant_tx
                    .send(msg.reply(e.to_string()))
                    .await
                    .unwrap_or_else(Self::err_log);
                self.server_status_manager
                    .lock()
                    .await
                    .failed_stop_server(&server)
                    .unwrap();
            }
        }
        if let Some(server) = save {
            if let Err(e) = self.save_server(&server, msg).await {
                self.bot_instant_tx
                    .send(msg.reply(e.to_string()))
                    .await
                    .unwrap_or_else(Self::err_log);
            }
        }
        Some(msg.reply("cmd exec finish.".into()))
    }

    pub async fn handle_nps_cmd(&self, ip: String, msg: &RecvMsg) -> Option<SendMsg> {
        if ip.parse::<std::net::IpAddr>().is_err() {
            return Some(msg.reply("ip format error".into()));
        }
        let nps_access = &self.config.nps;
        let region = Region::from_str(&nps_access.region).unwrap();
        let instance_id = &nps_access.instance_id;
        let target_protocol = &nps_access.protocol;
        let target_port = &nps_access.port;
        let mut firewallrules = self
            .client
            .lighthouse()
            .firewall()
            .describe_firewall_rules(&region, instance_id)
            .await
            .unwrap_or_default()
            .response
            .firewall_rule_set;
        firewallrules
            .iter_mut()
            .filter(|r| r.port == *target_port && r.protocol == *target_protocol)
            .for_each(|r| {
                r.cidr_block = ip.clone();
            });
        let content = match self
            .client
            .lighthouse()
            .firewall()
            .modify_firewall_rules(&region, instance_id, firewallrules)
            .await
        {
            Ok(_) => "Success modify firewall rules".to_string(),
            Err(e) => format!("modify firewall rules failed: {e}"),
        };
        Some(msg.reply(content))
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
                    save,
                } => {
                    self.handle_server_cmd(status, start, stop, save, &msg)
                        .await
                }
                Commands::Config { r#type: _type } => None,
                Commands::Nps { ip } => self.handle_nps_cmd(ip, &msg).await,
            };
            return res;
        }
        None
    }
    async fn check_cmd_auth(&self, cmd: &Self::Cmd, ori_msg: &RecvMsg, root_id: u64) -> bool {
        let white_list = &self.config.whitelist;
        let allow_act = cmd.sub.as_ref().is_some_and(|c| match c {
            Commands::Server { .. } => white_list.server.contains(&ori_msg.from_id),
            Commands::Config { .. } => ori_msg.from_id == root_id,
            Commands::Nps { .. } => white_list.nps.contains(&ori_msg.from_id),
        });
        debug!("is allowed cmd: {allow_act}");
        allow_act
    }
}
