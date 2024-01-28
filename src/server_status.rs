use std::{fmt::Display, path::Path};

use serde::{Deserialize, Serialize};

pub struct ServerManager {
    path: String,
    servers: Vec<Server>,
}

type ServerManagerResult<T> = Result<T, String>;
impl ServerManager {
    pub fn new(path: &Path) -> Self {
        let path = path.to_str().unwrap().to_string();
        let data = std::fs::read_to_string(&path).expect("read server status file failed");
        let servers: Vec<_> = serde_yaml::from_str(&data).expect("parse server status file failed");
        Self { path, servers }
    }

    pub fn list(&self, server: &str) -> ServerManagerResult<String> {
        let server = self.find_server_or_err(server)?;
        Ok(format!("{}", server))
    }

    pub fn get_instance_type(&self, server: &str) -> ServerManagerResult<String> {
        let server = self.find_server_or_err(server)?;
        Ok(server.instance_type.clone())
    }

    pub fn get_save_name(&self, server: &str) -> ServerManagerResult<Option<String>> {
        let server = self.find_server_or_err(server)?;
        Ok(server.save.clone())
    }

    pub fn get_server_ip(&self, server: &str) -> ServerManagerResult<Option<String>> {
        let server = self.find_server_or_err(server)?;
        Ok(server
            .ip_port
            .as_ref()
            .map(|ip_port| ip_port[0..ip_port.find(":").map_or(0, |x| x)].to_string()))
    }

    pub fn update_save_name(&mut self, server: &str, save_name: &str) -> ServerManagerResult<()> {
        let server = self.find_server_or_err_mut(server)?;
        server.save = Some(save_name.to_owned());
        Ok(())
    }

    pub fn check_server_status(&self, server: &str, status: &Status) -> ServerManagerResult<()> {
        let server = self.find_server_or_err(server)?;
        (&server.status == status).then(|| ()).ok_or(format!(
            "[Error] server current status: {}\n",
            server.status
        ))
    }

    pub fn create_server(&mut self, server: &str) -> ServerManagerResult<()> {
        let server = self.find_server_or_err_mut(server)?;
        server.status = Status::Creating;
        self.update()?;
        Ok(())
    }

    pub fn finish_creating_server(
        &mut self,
        server: &str,
        ip_port: &str,
        region: &str,
        instance_id: &str,
    ) -> ServerManagerResult<()> {
        let server = self.find_server_or_err_mut(server)?;
        server.status = Status::Running;
        server.ip_port = Some(ip_port.to_owned());
        server.region = Some(region.to_owned());
        server.instance_id = Some(instance_id.to_owned());
        self.update()?;
        Ok(())
    }

    pub fn failed_create_server(&mut self, server: &str) -> ServerManagerResult<()> {
        let server = self.find_server_or_err_mut(server)?;
        server.status = Status::Stopped;
        self.update()?;
        Ok(())
    }

    pub fn failed_stop_server(&mut self, server: &str) -> ServerManagerResult<()> {
        let server = self.find_server_or_err_mut(server)?;
        server.status = Status::Running;
        self.update()?;
        Ok(())
    }

    pub fn stop_server(&mut self, server: &str) -> ServerManagerResult<(String, String)> {
        let server = self.find_server_or_err_mut(server)?;
        let (region, id) = (
            server.region.clone().unwrap(),
            server.instance_id.clone().unwrap(),
        );
        server.status = Status::Stopping;
        self.update()?;
        Ok((region, id))
    }

    pub fn finish_stopping_server(&mut self, server: &str) -> ServerManagerResult<()> {
        let server = self.find_server_or_err_mut(server)?;
        server.status = Status::Stopped;
        server.ip_port = None;
        server.region = None;
        server.instance_id = None;
        self.update()?;
        Ok(())
    }

    fn find_server_or_err_mut(&mut self, server: &str) -> ServerManagerResult<&mut Server> {
        self.servers
            .iter_mut()
            .find(|s| s.name == server)
            .ok_or("[Error] server not found\n".into())
    }
    fn find_server_or_err(&self, server: &str) -> ServerManagerResult<&Server> {
        self.servers
            .iter()
            .find(|s| s.name == server)
            .ok_or("[Error] server not found\n".into())
    }

    fn update(&mut self) -> ServerManagerResult<()> {
        let data = serde_yaml::to_string(&self.servers)
            .map_err(|e| format!("[Error] serde_yaml err: {e}"))?;
        std::fs::write(&self.path, data).map_err(|e| format!("[Error] fs write err: {e}"))?;
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Server {
    pub name: String,
    pub status: Status,
    pub instance_type: String,
    pub save: Option<String>,
    pub ip_port: Option<String>,
    pub region: Option<String>,
    pub instance_id: Option<String>,
}

impl Display for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "存档{name}(当前服务器状态: {status}) ip: {ip_port} type: {instance_type}
            存档文件{save}
            ",
            name = self.name,
            status = self.status,
            ip_port = self.ip_port.as_deref().unwrap_or("无"),
            instance_type = self.instance_type,
            save = self.save.as_deref().unwrap_or("无"),
        )
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum Status {
    Creating,
    Running,
    Stopping,
    Stopped,
}

impl Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let status = match self {
            Status::Creating => "Creating",
            Status::Running => "Running",
            Status::Stopping => "Stopping",
            Status::Stopped => "Stopped",
        };
        write!(f, "{}", status)
    }
}
