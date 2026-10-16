use config::{Config, ConfigError, Environment, File};
use serde_derive::Deserialize;
use std::{env, path::PathBuf};

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct AppConfig {
    pub grpcport: String,
    pub username: String,
    pub password: String,
    pub baseurl: String,
    pub urlget: Option<String>,
    pub urlfilter: Option<Vec<(String, Vec<String>)>>,
    pub entries: u32,
    pub filter1: String,
    pub filter2: String,
    pub urlput: String,
    pub printmode: bool,
    pub checkmode: bool,
    pub filemode: bool,
    pub filelist: String,
    pub timeout: u64,
    pub sleep: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GrpcServer {
    pub port: String,
    pub host: String,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(dead_code)]
pub struct Database {
    pub url: String,
    pub user: String,
    pub password: String,
    pub namespace: String,
    pub database: String,
    pub token: String,
    pub jwt: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct Settings {
    pub grpc: AppConfig,
    pub grpc_server: GrpcServer,
    pub database: Database,
    pub db: bool,
    pub loglevel: String,
}

impl Settings {
    pub(crate) fn new() -> Result<Self, ConfigError> {
        let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let config_dir = current_dir.join("config");
        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());

        let default_file = config_dir.join("default");
        let mode_file = config_dir.join(&run_mode);
        let local_file = config_dir.join("local");

        let s = Config::builder()
            .add_source(File::with_name(default_file.to_str().unwrap_or("config/default")))
            .add_source(
                File::with_name(mode_file.to_str().unwrap_or("config/development")).required(false),
            )
            .add_source(
                File::with_name(local_file.to_str().unwrap_or("config/local")).required(false),
            )
            .add_source(Environment::with_prefix("app"))
            .build()?;

        s.try_deserialize()
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use super::*;

    #[test]
    fn config_parse_test() -> Result<(), Box<dyn std::error::Error>> {
        let settings = Settings::new();
        assert!(settings.is_ok(), "Failed to parse settings: {settings:?}");
        Ok(())
    }

    #[test]
    fn url_converter() -> Result<(), Box<dyn std::error::Error>> {
        let settg = Settings::new()?.grpc_server;
        let addr: Result<SocketAddr, _> = format!("{}:{}", settg.host, settg.port).parse();
        assert!(addr.is_ok(), "Failed to parse address");
        Ok(())
    }
}
