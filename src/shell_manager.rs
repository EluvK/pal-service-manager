use std::{io::Read, net::TcpStream, path::Path};

use tracing::debug;

use crate::config::SshConfig;

#[derive(Debug)]
pub enum Script {
    /// install_server.sh
    InstallServer,
    /// restore_save.sh
    RestoreSave,
    /// start_server.sh
    StartServer,
    /// backup_save.sh
    BackupSave,
}

#[derive(Debug)]
pub struct ShellManager {
    pub ssh_config: SshConfig,
}

impl ShellManager {
    pub fn new(ssh_config: SshConfig) -> Self {
        Self { ssh_config }
    }

    pub async fn run(&self, ip: &str, script: Script) -> anyhow::Result<String> {
        let user = &self.ssh_config.user;
        let prikey_path = &self.ssh_config.prikey;

        let tcp = TcpStream::connect(format!("{ip}:22"))?;
        let mut sess = ssh2::Session::new()?;
        sess.set_tcp_stream(tcp);
        sess.handshake()?;
        sess.userauth_pubkey_file(user, None, &Path::new(prikey_path), None)?;
        sess.authenticated()
            .then(|| debug!("ssh2 authed"))
            .ok_or(anyhow::anyhow!("ssh2 auth failed"))?;

        let script_name = match script {
            Script::InstallServer => "install_server.sh",
            Script::RestoreSave => "restore_save.sh",
            Script::StartServer => "start_server.sh",
            Script::BackupSave => "backup_save.sh",
        };

        let mut channel = sess.channel_session()?;
        channel.exec(&format!(
            "(sh /home/{user}/psm/scripts/{script_name} >> /tmp/shell_log.log 2>&1 &)"
        ))?;

        const CHECK_INTERVAL: u64 = 5;
        loop {
            let mut channel = sess.channel_session()?;
            channel.exec(&format!(
                "ps -ef | grep {script_name} | grep -v grep | wc -l",
            ))?;
            let mut process_cnt = String::new();
            channel.read_to_string(&mut process_cnt)?;
            if process_cnt.trim() == "0" {
                break;
            }
            debug!(" - running...");
            tokio::time::sleep(tokio::time::Duration::from_secs(CHECK_INTERVAL)).await;
        }

        let res = {
            let mut channel = sess.channel_session()?;
            channel.exec("tail -n 1 /tmp/shell_log.log")?;
            let mut logs = String::new();
            channel.read_to_string(&mut logs)?;
            debug!(" -logs: {}", logs);
            channel.close()?;
            debug!(" -status: {}", channel.exit_status()?);
            logs
        };
        Ok(res)
    }
}
