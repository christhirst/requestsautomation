mod config;
mod datapolars;
mod db;
mod error;
mod grpcserver;
mod http;
mod model;
mod types;
mod wrangle;
use std::env;

use error::CliError;
use grpcserver::proto;
use proto::user_server::UserServer;

use tonic::transport::Server;
use tracing::info;

use crate::config::Settings;

#[tokio::main]
async fn main() -> Result<(), CliError> {
    //CONFIG read
    let conf = Settings::new().unwrap();
    //STRACING setup
    let env_loglevl = env::var("LOGLEVEL").unwrap_or(conf.loglevel);

    let loglevel = match env_loglevl.as_str() {
        "ERROR" => tracing::Level::ERROR,
        "WARN" => tracing::Level::WARN,
        "INFO" => tracing::Level::INFO,
        "DEBUG" => tracing::Level::DEBUG,
        "TRACE" => tracing::Level::TRACE,
        _ => tracing::Level::INFO,
    };
    let subscriber = tracing_subscriber::fmt().with_max_level(loglevel).finish();
    //use that subscriber to process traces emitted after this point
    tracing::subscriber::set_global_default(subscriber)?;

    let conf = Settings::new().unwrap();

    let url = &conf.grpc.baseurl.clone();
    let urlget = &conf.grpc.urlget.clone();
    let urlput = &conf.grpc.urlput.clone();
    let geturl = format!("{}{}{}", url, urlput, urlget);

    info!(
        "Version: {:?}, LOGLEVEL: {:?}, URL: {:?}",
        "v0.0.29", env_loglevl, geturl
    );

    let settg = Settings::new().unwrap().grpc_server;
    let addr = format!("{}:{}", settg.host, settg.port).parse();

    info!("gRPC Setting up");
    let calc = grpcserver::UserService::new().await?;

    info!("gRPC gRPC reflection");
    let service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(grpcserver::proto::FILE_DESCRIPTOR_SET)
        .build_v1()?;

    info!("gRPC serving at: {:?}", addr);
    Server::builder()
        .accept_http1(true)
        //.layer(tower_http::cors::CorsLayer::permissive())
        .add_service(service)
        .add_service(UserServer::new(calc))
        //.add_service(tonic_web::enable(CalculatorServer::new(calc)))
        //.add_service(AdminServer::with_interceptor(admin, check_auth))
        .serve(addr.unwrap())
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /* #[test]
    fn urlsbuilder_test() -> Result<(), Box<dyn std::error::Error>> {
        //let filename1 = "Config.toml";
        //let conf = load_or_initialize(filename1).unwrap();
        let conf = Settings::new().unwrap().grpc;
        /* let urlresult = format!(
            "{}/{}+eq+{}",
            conf.baseurl, conf.urlfilter[0].0, conf.urlfilter[0].1[0]
        ); */

        let n = httprequests::urlsbuilder(&conf.baseurl, &conf.urlfilter);
        println!("{n:?}");
        println!("--------------");

        //assert_eq!(urlresult, n);
        Ok(())
    } */

    /*  #[ignore]
    #[test]
    fn fileappend_test() -> Result<(), Box<dyn std::error::Error>> {
        let filename1 = "Config.toml";
        let conf = load_or_initialize(filename1).unwrap();
        let urlresult = format!(
            "{}/{}+eq+{}",
            conf.baseurl, conf.urlfilter[0].0, conf.urlfilter[0].1[0]
        );

        let port = conf.grpcport;

        let addr: std::net::SocketAddr = "[::1]:".push_str(port).parse()?;

        let n = httprequests::urlsbuilder(&conf.baseurl, &conf.urlfilter);
        println!("{n:?}");
        println!("--------------");

        //assert_eq!(urlresult, n);

        //grpc server
        let calc = UserService::default();

        let service = tonic_reflection::server::Builder::configure()
            .register_encoded_file_descriptor_set(proto::FILE_DESCRIPTOR_SET)
            .build()?;

        Ok(())
    } */
}
