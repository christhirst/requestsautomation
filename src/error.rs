use polars::prelude::*;
use reqwest::{self, Error as rError};
use thiserror::Error;
use tonic::{Code, Status};
use tracing::subscriber::SetGlobalDefaultError;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("Entity '{entity}' with id {id} not found")]
    EntityNotFound { entity: &'static str, id: i64 },
    //ConfigError(ConfigError),
    #[error("Entity  not found")]
    Er(Box<dyn std::error::Error>),
    #[error("Entity  not found")]
    FailedToCreatePool(String),
    #[error("Entity  not found")]
    PE(PolarsError),
    #[error("Entity  not found")]
    RError(rError),
    #[error("Entity  not found")]
    GlobalDefaultError(SetGlobalDefaultError),
    #[error("unknown data store error")]
    DBError(#[from] surrealdb::Error),
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

/* impl From<ConfigError> for CliError {
    fn from(err: ConfigError) -> CliError {
        CliError::ConfigError(err)
    }
} */

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
