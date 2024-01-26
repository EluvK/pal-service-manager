use std::{fmt::Display, path::Path};

use serde::{Deserialize, Serialize};

pub struct ServerManager {
    path: String,
    servers: Vec<Server>,
}

impl ServerManager {
    pub fn new(path: &Path) -> Self {
        let path = path.to_str().unwrap().to_string();
        let data = std::fs::read_to_string(&path).expect("read server status file failed");
        let servers = serde_yaml::from_str(&data).expect("parse server status file failed");
        Self { path, servers }
    }

    pub fn list(&self, server: Option<String>) -> String {
        let mut list = String::new();
        if let Some(server) = server {
            if let Some(server) = self.servers.iter().find(|s| s.name == server) {
                list.push_str(&format!("{}\n", server));
            }
        } else {
            for server in &self.servers {
                list.push_str(&format!("{}\n", server));
            }
        }
        list
    }

    pub fn create_server(&mut self, name: &str) -> anyhow::Result<()> {
        let server = self
            .servers
            .iter_mut()
            .find(|s| s.name == name)
            .ok_or_else(|| anyhow::anyhow!("server not found"))?;
        server.status = Status::Creating;
        self.update()?;
        Ok(())
    }

    pub fn finish_creating_server(&mut self, name: &str, ip_port: &str) -> anyhow::Result<()> {
        let server = self
            .servers
            .iter_mut()
            .find(|s| s.name == name)
            .ok_or_else(|| anyhow::anyhow!("server not found"))?;
        server.status = Status::Running;
        server.ip_port = Some(ip_port.to_string());
        self.update()?;
        Ok(())
    }

    pub fn stop_server(&mut self, name: &str) -> anyhow::Result<()> {
        let server = self
            .servers
            .iter_mut()
            .find(|s| s.name == name)
            .ok_or_else(|| anyhow::anyhow!("server not found"))?;
        server.status = Status::Stopping;
        self.update()?;
        Ok(())
    }

    pub fn finish_stopping_server(&mut self, name: &str) -> anyhow::Result<()> {
        let server = self
            .servers
            .iter_mut()
            .find(|s| s.name == name)
            .ok_or_else(|| anyhow::anyhow!("server not found"))?;
        server.status = Status::Stopped;
        server.ip_port = None;
        self.update()?;
        Ok(())
    }

    fn update(&mut self) -> anyhow::Result<()> {
        let data = serde_yaml::to_string(&self.servers)?;
        std::fs::write(&self.path, data)?;
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Server {
    pub name: String,
    pub status: Status,
    pub ip_port: Option<String>,
}

impl Display for Server {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{name} {status} {ip_port}",
            name = self.name,
            status = self.status,
            ip_port = self.ip_port.as_deref().unwrap_or("None"),
        )
    }
}

#[derive(Debug, Deserialize, Serialize)]
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
