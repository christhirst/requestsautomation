mod config;
mod datapolars;
mod error;
mod grpcserver;
mod httprequests;
mod model;

use error::CliError;
use grpcserver::proto;
use proto::user_server::UserServer;

use tonic::transport::Server;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), CliError> {
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    // use that subscriber to process traces emitted after this point
    tracing::subscriber::set_global_default(subscriber)?;

    let file = "Config.toml";
    let conf = config::confload(file)?;
    let url = conf.baseurl;
    let urlget = conf.urlget;
    let urlput = conf.urlput;

    info!("Version: {:?}", "v0.0.21");

    let geturl = format!("{}{}{}", url, urlput, urlget);

    /*  let client = reqwest::Client::new();
    let json_data = r#"{"action": "retry"}"#;
    let json_data = r#"{"action": "manualComplete"}"#; */
    info!("{}", geturl);

    let addr = "[::1]:50051".parse().unwrap();

    //let state = State::default();

    let calc = grpcserver::UserService::default();

    let service = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(grpcserver::proto::FILE_DESCRIPTOR_SET)
        .build()
        .unwrap();

    Server::builder()
        .accept_http1(true)
        //.layer(tower_http::cors::CorsLayer::permissive())
        .add_service(service)
        .add_service(UserServer::new(calc))
        //.add_service(tonic_web::enable(CalculatorServer::new(calc)))
        //.add_service(AdminServer::with_interceptor(admin, check_auth))
        .serve(addr)
        .await
        .unwrap();

    Ok(())
}

#[cfg(test)]
mod tests {
    use grpcserver::UserService;

    use self::config::load_or_initialize;

    use super::*;

    #[test]
    fn urlsbuilder_test() -> Result<(), Box<dyn std::error::Error>> {
        let filename1 = "Config.toml";
        let conf = load_or_initialize(filename1).unwrap();
        let urlresult = format!(
            "{}/{}+eq+{}",
            conf.baseurl, conf.urlfilter[0].0, conf.urlfilter[0].1[0]
        );

        let n = httprequests::urlsbuilder(&conf.baseurl, &conf.urlfilter);
        println!("{n:?}");
        println!("--------------");

        //assert_eq!(urlresult, n);
        Ok(())
    }

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
