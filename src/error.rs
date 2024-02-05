use thiserror::Error;
use tokio::sync::mpsc::error::SendError;

use crate::server_status;

#[derive(Error, Debug)]
pub enum PSMError {
    #[error("Server Manager error: {0}")]
    ServerManagerError(#[from] server_status::ServerManagerError),

    #[error("CSP client error: {0}")]
    CSPClientError(String),

    #[error("Send error: {0}")]
    SendError(String),

    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
    #[error("Config error: {0}")]
    Config(#[from] config::ConfigError),
    #[error("Serde error: {0}")]
    Serde(#[from] serde_yaml::Error),
    #[error("Clap error: {0}")]
    Clap(#[from] clap::Error),
    #[error("Tokio error: {0}")]
    Tokio(#[from] tokio::task::JoinError),
    #[error("Anyhow error: {0}")]
    Anyhow(#[from] anyhow::Error),
}

impl<T> From<SendError<T>> for PSMError {
    fn from(e: SendError<T>) -> Self {
        PSMError::SendError(format!("{}", e))
    }
}
