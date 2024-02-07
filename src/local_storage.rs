use opendal::{
    services::{Fs, Sftp},
    Operator,
};
use serde::Deserialize;

use crate::config::SshConfig;

#[derive(Debug, Deserialize, Clone)]
pub struct LocalSaveStorageConfig {
    local_dir: String,
    remote_dir: String,
}

#[derive(Debug)]
pub struct LocalStorage {
    config: LocalSaveStorageConfig,
}

impl LocalStorage {
    pub fn new(config: LocalSaveStorageConfig) -> Self {
        Self { config }
    }
    fn build_local_op(&self) -> anyhow::Result<Operator> {
        let mut fs = Fs::default();
        fs.root(&self.config.local_dir);
        Ok(Operator::new(fs)?.finish())
    }
    fn build_remote_sftp(&self, ssh: &SshConfig, ip: &str) -> anyhow::Result<Operator> {
        let endpoint = format!("ssh://{}@{}:22", ssh.user, ip);
        let mut sftp = Sftp::default();
        sftp.root(&self.config.remote_dir)
            .endpoint(&endpoint)
            .key(&ssh.prikey)
            .user(&ssh.user)
            .known_hosts_strategy("Accept");
        Ok(Operator::new(sftp)?.finish())
    }

    pub async fn upload_scripts(&self, ssh: &SshConfig, ip: &str) -> anyhow::Result<()> {
        let local_op = self.build_local_op()?;
        let remote_op = self.build_remote_sftp(ssh, ip)?;

        let files = &[
            "install_server.sh",
            "restore_save.sh",
            "start_server.sh",
            "backup_save.sh",
        ];
        for file in files {
            let content = local_op.read(&format!("/scripts/{}", file)).await?;
            remote_op
                .write(&format!("/scripts/{}", file), content)
                .await?;
        }
        Ok(())
    }

    pub async fn upload_saves(
        &self,
        save_name: &str,
        ssh: &SshConfig,
        ip: &str,
    ) -> anyhow::Result<()> {
        let local_op = self.build_local_op()?;
        let remote_op = self.build_remote_sftp(ssh, ip)?;

        let content = local_op.read(&format!("/saves/{}", save_name)).await?;
        remote_op
            .write(&format!("/saves/{}", save_name), content)
            .await?;
        Ok(())
    }

    pub async fn download_saves(
        &self,
        save_name: &str,
        ssh: &SshConfig,
        ip: &str,
    ) -> anyhow::Result<()> {
        let local_op = self.build_local_op()?;
        let remote_op = self.build_remote_sftp(ssh, ip)?;
        let content = remote_op.read(&format!("/saves/{}", save_name)).await?;
        local_op
            .write(&format!("/saves/{}", save_name), content)
            .await?;

        Ok(())
    }
}
