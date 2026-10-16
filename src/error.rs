use config::ConfigError;
use polars::prelude::*;
use reqwest::Error as ReqwestError;
use thiserror::Error;
use tonic::{Code, Status};
use tracing::subscriber::SetGlobalDefaultError;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("Entity '{entity}' with id {id} not found")]
    EntityNotFound { entity: &'static str, id: i64 },

    #[error("Invalid action: {0}")]
    InvalidAction(String),

    #[error("Configuration error: {0}")]
    Config(#[from] ConfigError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Polars error: {0}")]
    PE(#[from] PolarsError),

    #[error("HTTP request error: {0}")]
    RError(#[from] ReqwestError),

    #[error("Tracing subscriber error: {0}")]
    GlobalDefaultError(#[from] SetGlobalDefaultError),

    #[error("Database error: {0}")]
    DBError(#[from] surrealdb::Error),

    #[error("Tonic reflection error: {0}")]
    TonicReflection(#[from] tonic_reflection::server::Error),

    #[error("Tonic transport error: {0}")]
    TonicTransport(#[from] tonic::transport::Error),
}

impl From<CliError> for tonic::Status {
    fn from(val: CliError) -> Self {
        match &val {
            CliError::EntityNotFound { .. } => Status::new(Code::NotFound, val.to_string()),
            CliError::InvalidAction(_) => Status::new(Code::InvalidArgument, val.to_string()),
            CliError::RError(_) => Status::new(Code::Unavailable, val.to_string()),
            CliError::PE(_) => Status::new(Code::InvalidArgument, val.to_string()),
            _ => Status::new(Code::Internal, val.to_string()),
        }
    }
}
