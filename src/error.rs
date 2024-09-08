use crate::config::ConfigError;
use polars::prelude::*;
use reqwest::{self, Error as rError};
use tonic::{Code, Status};
use tracing::subscriber::SetGlobalDefaultError;

#[derive(Debug)]
pub enum CliError {
    EntityNotFound { entity: &'static str, id: i64 },
    ConfigError(ConfigError),
    Er(Box<dyn std::error::Error>),
    FailedToCreatePool(String),
    PE(PolarsError),
    RError(rError),
    GlobalDefaultError(SetGlobalDefaultError),
}

impl From<CliError> for tonic::Status {
    fn from(val: CliError) -> Self {
        Status::new(Code::InvalidArgument, format!("{:?}", val))
    }
}

impl From<PolarsError> for CliError {
    fn from(err: PolarsError) -> CliError {
        CliError::PE(err)
    }
}

impl From<ConfigError> for CliError {
    fn from(err: ConfigError) -> CliError {
        CliError::ConfigError(err)
    }
}

impl From<rError> for CliError {
    fn from(err: rError) -> CliError {
        CliError::RError(err)
    }
}

impl From<SetGlobalDefaultError> for CliError {
    fn from(err: SetGlobalDefaultError) -> CliError {
        CliError::GlobalDefaultError(err)
    }
}
