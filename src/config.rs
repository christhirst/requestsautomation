use config::{Config, ConfigError, Environment, File};
use serde_derive::Deserialize;
use std::env;

#[derive(Debug, Deserialize, Clone)]
#[allow(unused)]
pub struct AppConfig {
    pub grpcport: String,
    pub username: String,
    pub password: String,
    pub baseurl: String,
    pub urlget: String,
    pub urlfilter: Vec<(String, Vec<String>)>,
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

#[derive(Debug, Deserialize)]
#[allow(unused)]
pub struct GrpcServer {
    pub port: String,
    pub host: String,
}

#[derive(Debug, Deserialize, Clone)]
#[allow(unused)]
pub struct Database {
    pub url: String,
    pub user: String,
    pub password: String,
    pub namespace: String,
    pub database: String,
    pub token: String,
    pub jwt: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(unused)]
pub(crate) struct Settings {
    pub grpc: AppConfig,
    pub grpc_server: GrpcServer,
    pub database: Database,
    pub db: bool,         // Flag to indicate if DB is used
    pub loglevel: String, // Log level for the application
}

impl Settings {
    pub(crate) fn new() -> Result<Self, ConfigError> {
        let paths;
        match env::current_dir() {
            Ok(path) => {
                println!("Current directory: {}", path.display());
                paths = path;
            }
            Err(e) => {
                paths = "".into();
                eprintln!("Error getting current directory: {}", e)
            }
        }
        let paths = paths.join("config");
        let run_mode = env::var("RUN_MODE").unwrap_or_else(|_| "development".into());

        let s = Config::builder()
            // Start off by merging in the "default" configuration file
            .add_source(File::with_name(paths.join("default").to_str().unwrap()))
            // Add in the current environment file
            // Default to 'development' env
            // Note that this file is _optional_
            .add_source(
                File::with_name(&format!("{}/{}", paths.display(), run_mode)).required(false),
            )
            // Add in a local configuration file
            // This file shouldn't be checked in to git
            .add_source(File::with_name(&format!("{}/locals", paths.display())).required(false))
            // Add in settings from the environment (with a prefix of APP)
            // Eg.. `APP_DEBUG=1 ./target/app` would set the `debug` key
            .add_source(Environment::with_prefix("app"))
            // You may also programmatically change settings
            //.set_override("database.url", "postgres://")?
            .build()?;

        // Now that we're done, let's access our configuration
        /* println!("debug: {:?}", s.get_bool("debug"));
        println!("database: {:?}", s.get::<String>("database.url")); */

        // You can deserialize (and thus freeze) the entire configuration as
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
        // Print out our settings
        println!("{settings:?}");
        //panic!("Test failed, this is a panic to test the error handling in the test framework");
        assert!(settings.is_ok(), "Failed to parse settings");
        Ok(())
    }

    /* #[test]
    fn url_converter() -> Result<(), Box<dyn std::error::Error>> {
        let settg = Settings::new().unwrap().grpc_server;
        print!("Address: {:?}", settg);
        let addr: Result<SocketAddr, _> = format!("{}:{}", settg.host, settg.port).parse();
        print!("Address: {:?}", addr);

        assert!(addr.is_ok(), "Failed to parse address");
        Ok(())
    }  */
}
