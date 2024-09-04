mod config;
mod datapolars;
mod grpcserver;
mod httprequests;
use config::ConfigError;
use grpcserver::proto;
use polars::functions::concat_df_horizontal;
use proto::user_server::{User, UserServer};

use polars::prelude::*;
//test
//polars::prelude::NamedFrom<std::vec::Vec<serde_json::Value>
use reqwest::{self, header::CONTENT_TYPE, Client, Error as rError, Response};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    fmt,
    fs::{self, OpenOptions},
    path::Path,
    thread,
    time::Duration,
};
use tonic::{transport::Server, Code, Status};
use tracing::{info, subscriber::SetGlobalDefaultError};

use crate::datapolars::pl_vstr_to_selects;

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Action {
    pub action: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resp {
    pub links: Vec<Link>,
    pub id: String,
    pub status: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub links: Vec<Link>,
    pub count: i64,
    pub has_more: bool,
    pub total_result: i64,
    //different types
    pub accounts: Option<Vec<Account>>,
    pub tasks: Option<Vec<Task>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Properties {
    pub count: i64,
    pub has_more: bool,
    pub total_result: i64,
    pub users: Users,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Users {
    #[serde(rename = "type")]
    pub type_field: String,
    pub items: Items<String>,
}
/* #[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootAccount {
    pub links: Vec<Link>,
    pub count: i64,
    pub has_more: bool,
    pub total_result: i64,
    pub accounts: Vec<Account>,
    pub tasks: Vec<Task>,
} */

#[derive(Debug, Clone, Serialize, Deserialize)]
enum Roots {
    Root(Root),
    //RootAccount(RootAccount),
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Urlfilter {
    pub f1: F1,
    pub f2: F2,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct F1 {
    pub test: Vec<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct F2 {
    pub test: Vec<String>,
}

impl fmt::Display for Root {
    // This trait requires `fmt` with this exact signature.
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Write strictly the first element into the supplied output
        // stream: `f`. Returns `fmt::Result` which indicates whether the
        // operation succeeded or failed. Note that `write!` uses syntax which
        // is very similar to `println!`.
        write!(f, "{}", self.count)
    }
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub rel: String,
    pub href: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub links: Vec<Link2>,
    pub fields: Vec<Field>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Link2 {
    pub rel: String,
    pub href: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub name: String,
    pub value: Value,
}

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
struct Items<T> {
    items: Vec<T>,
}
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct APIResponse {
    pub user_id: i64,
    pub id: i64,
    pub title: String,
    pub completed: bool,
}

//  structs for option price query
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct OptionResponse {
    pub user_id: i64,
    pub id: i64,
    pub title: String,
    pub completed: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub links: Vec<Link2>,
    pub id: String,
    pub name: String,
    pub user_id: String,
    pub app_instance_id: String,
    pub request_id: String,
    pub fields: Vec<Field>,
}

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

impl Into<tonic::Status> for CliError {
    fn into(self) -> tonic::Status {
        Status::new(Code::InvalidArgument, "name is invalid")
    }
}

impl From<PolarsError> for CliError {
    fn from(err: PolarsError) -> CliError {
        CliError::PE(err)
    }
}

impl From<config::ConfigError> for CliError {
    fn from(err: config::ConfigError) -> CliError {
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

    info!("Version: {:?}", "v0.0.20");

    let geturl = format!("{}{}{}", url, urlput, urlget);

    let client = reqwest::Client::new();
    let json_data = r#"{"action": "retry"}"#;
    let json_data = r#"{"action": "manualComplete"}"#;
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

    #[test]
    async fn fileappend_test() -> Result<(), Box<dyn std::error::Error>> {
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

        Server::builder()
            .accept_http1(true)
            //.layer(tower_http::cors::CorsLayer::permissive())
            .add_service(service)
            .add_service(UserServer::new(calc))
            //.add_service(tonic_web::enable(CalculatorServer::new(calc)))
            //.add_service(AdminServer::with_interceptor(admin, check_auth))
            .serve(addr)
            .await?;

        Ok(())
    }
}
