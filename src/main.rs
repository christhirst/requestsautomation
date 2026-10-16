mod config;
pub mod dataendpoint;
mod datapolars;
mod db;
mod error;
mod file;
mod grpcserver;
mod http;
mod model;
mod types;
mod wrangle;

use std::{env, net::SocketAddr};

use error::CliError;
use grpcserver::proto;
use proto::user_server::UserServer;
use tonic::transport::Server;
use tracing::info;

use crate::config::Settings;

#[tokio::main]
async fn main() -> Result<(), CliError> {
    let conf = Settings::new()?;

    let env_loglevel = env::var("LOGLEVEL").unwrap_or_else(|_| conf.loglevel.clone());
    let loglevel = match env_loglevel.to_uppercase().as_str() {
        "ERROR" => tracing::Level::ERROR,
        "WARN" => tracing::Level::WARN,
        "INFO" => tracing::Level::INFO,
        "DEBUG" => tracing::Level::DEBUG,
        "TRACE" => tracing::Level::TRACE,
        _ => tracing::Level::INFO,
    };

    let subscriber = tracing_subscriber::fmt().with_max_level(loglevel).finish();
    tracing::subscriber::set_global_default(subscriber)?;

    let geturl = format!(
        "{}{}{}",
        conf.grpc.baseurl,
        conf.grpc.urlput,
        conf.grpc.urlget.as_deref().unwrap_or_default()
    );

    info!(
        "Version: {:?}, LOGLEVEL: {:?}, OIM Endpoint: {:?}",
        env!("CARGO_PKG_VERSION"),
        env_loglevel,
        geturl
    );

    let addr: SocketAddr = format!("{}:{}", conf.grpc_server.host, conf.grpc_server.port)
        .parse()
        .expect("Invalid gRPC server host:port address");

    info!("Initializing gRPC UserService");
    let user_service = grpcserver::UserService::new(conf).await?;

    info!("Configuring gRPC server reflection");
    let reflection_service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(grpcserver::proto::FILE_DESCRIPTOR_SET)
        .build_v1()?;

    info!("gRPC server listening on {}", addr);
    Server::builder()
        .accept_http1(true)
        .add_service(reflection_service)
        .add_service(UserServer::new(user_service))
        .serve(addr)
        .await?;

    Ok(())
}
