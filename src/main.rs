mod config;
mod datapolars;
mod httprequests;

use axum::body;
use config::ConfigError;
use polars::functions::concat_df_horizontal;
async fn provTasks(
    &self,
    request: tonic::Request<proto::UserRequest>,
) -> Result<tonic::Response<proto::UserResponse>, tonic::Status>;
use polars::prelude::*;

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
    pub tasks: Vec<Task>,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RootAccount {
    pub links: Vec<Link>,
    pub count: i64,
    pub has_more: bool,
    pub total_result: i64,
    pub accounts: Vec<Account>,
}

enum Roots {
    Root(Root),
    RootAccount(RootAccount),
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

#[derive(Serialize, Deserialize, Debug)]
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
    //get Users
    // /iam/governance/selfservice/api/v1/users
    // Get all accounts for a user.

    // Get users from LDAP

    // /iam/governance/selfservice/api/v1/accounts
    // Delete account based on account id
    // /iam/governance/selfservice/api/v1/accounts/{accountid}

    // Get provisioning task with filter
    // /iam/governance/selfservice/api/v1/provtasks/{taskid}

    let subscriber = tracing_subscriber::FmtSubscriber::new();
    // use that subscriber to process traces emitted after this point
    tracing::subscriber::set_global_default(subscriber)?;

    let file = "Config.toml";
    let conf = config::confload(file)?;
    let url = conf.baseurl;
    let urlget = conf.urlget;
    let urlput = conf.urlput;
    let urlfilter = conf.urlfilter;
    let username = conf.username;
    let password = conf.password;
    let entries = conf.entries;
    let filter1 = conf.filter1;
    let filter2 = conf.filter2;
    let checkmode = conf.checkmode;
    let printmode = conf.printmode;
    let filemode = conf.filemode;

    info!("Version: {:?}", "v0.0.20");

    let geturl = format!("{}{}{}", url, urlput, urlget);

    let client = reqwest::Client::new();
    let json_data = r#"{"action": "retry"}"#;
    let json_data = r#"{"action": "manualComplete"}"#;
    info!("{}", geturl);

    if filemode {
        let mut tasksdone: Vec<Resp> = vec![];
        let tasksl = CsvReader::from_path("path.csv").unwrap().finish().unwrap()
            ["Process Instance.Task Details.Key"]
            .as_list();
        let mut retry: i32 = 0;
        for i in &tasksl {
            if checkmode {
                break;
            }
            let o = i.ok_or(CliError::EntityNotFound { entity: "", id: 1 })?;
            let id = o.get(0).unwrap();
            info!("{}", id);

            let puturl = format!("{}{}{}{}", url, urlput, "/", id);
            info!("PutUrl: {}", puturl);

            let mut status: u16 = 0;
            let mut json: Resp;
            while retry < 0 || status != 200 {
                retry += 1;
                match httprequests::retrycall(
                    &client,
                    puturl.clone(),
                    json_data.to_owned(),
                    username.clone(),
                    password.clone(),
                )
                .await
                {
                    Ok(response) => {
                        status = response.status().as_u16();
                        json = response.json().await?;
                        tasksdone.push(json);
                    }
                    Err(e) => {
                        info!("Retry failed: {e:?}");
                        thread::sleep(Duration::from_secs(3));
                    }
                }
            }

            let df = CsvReader::from_path("path.csv").unwrap().finish().unwrap();
            let length = df["Process Instance.Task Details.Key"].len() as u32;

            if status == 200 {
                let mut df_a = df.clone().lazy().slice(1, length - 1).collect()?;
                let mut file = std::fs::File::create("path.csv").unwrap();
                CsvWriter::new(&mut file).finish(&mut df_a).unwrap();

                retry += 1;
            }

            thread::sleep(Duration::from_secs(1));
        }
        info!("{:?}", tasksdone);
    } else {
        let urllist = httprequests::urlsbuilder(&url, &urlfilter);
        info!("URLBUILDER: {:?}", &urllist);

        for buildurl in urllist {
            let newurl = format!("{}{}", geturl, buildurl);
            info!("Request data from: {:?}", newurl);

            //new data from rest api
            let data =
                httprequests::get_data(&client, &newurl, &username, &password, entries).await?;

            //get header data
            let mut hm: HashMap<String, Series> =
                datapolars::getheaders(&client, &geturl, &username, &password).await?;

            let data = datapolars::fillseries(data, &mut hm).clone();

            let mut df_append = DataFrame::default();
            for (_i, v) in data {
                let df = v.into_frame();
                df_append = concat_df_horizontal(&[df_append, df])?;
            }

            let df_header = vec![
                "Process Instance.Task Information.Creation Date",
                "Objects.Name",
                "Process Instance.Task Details.Key",
                "Process Definition.Tasks.Task Name",
                "Process Instance.Task Information.Target User",
            ];

            //only select some columns
            let df = pl_vstr_to_selects(df_append, df_header)?;
            let mut out = datapolars::get_data(df, &filter1, &filter2)?;

            let tasks = out["Process Instance.Task Details.Key"].as_list();
            //let _ids = out["Process Instance.Task Information.Target User"].as_list();

            if printmode {
                let fileexists = !Path::new("path.csv").exists();
                let mut file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("path.csv")
                    .unwrap();

                CsvWriter::new(&mut file)
                    .include_header(fileexists)
                    .finish(&mut out)
                    .unwrap();

                //file.write_all(b"\n").unwrap();

                let mut dfa = out
                    .clone()
                    .lazy()
                    .select([col("Process Instance.Task Information.Target User")])
                    .collect()?;

                let mut file = std::fs::File::create("ids.csv").unwrap();
                CsvWriter::new(&mut file).finish(&mut dfa).unwrap();

                let contents =
                    fs::read_to_string("ids.csv").expect("Should have been able to read the file");
                let splitted: Vec<&str> = contents.split('\n').collect();
                println!("Ids: {:?}", splitted);
            } else {
                let mut tasksdone: Vec<Resp> = vec![];
                for i in &tasks {
                    if checkmode {
                        break;
                    }
                    let o = i.ok_or(CliError::EntityNotFound { entity: "", id: 1 })?;
                    let id = o.get(0).unwrap();
                    info!("Retry id: {}", id);

                    let puturl = format!("{}{}{}{}", url, urlput, "/", id);
                    info!("Request put: {}", puturl);

                    let response = httprequests::retrycall(
                        &client,
                        puturl,
                        json_data.to_owned(),
                        username.clone(),
                        password.clone(),
                    )
                    .await?;
                    let json: Resp = response.json().await?;
                    tasksdone.push(json);
                    //info!("{:?}", json);
                    //info!("{}", json.id);
                    thread::sleep(Duration::from_secs(1));
                }
                info!("Tasks done: {:?}", tasksdone);
            }
        }
    }
    Ok(())
}

use proto::user_server::{User, UserServer};

mod proto {
    tonic::include_proto!("grpc");

    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("user_descriptor");
}

type State = std::sync::Arc<tokio::sync::RwLock<u64>>;

#[derive(Debug, Default)]
struct UserService {
    state: State,
}
//#[tonic::async_trait]
impl User for UserService {
    async fn list(
        &self,
        request: tonic::Request<proto::UserRequest>,
    ) -> Result<tonic::Response<proto::UserResponse>, tonic::Status> {
        let input = request.get_ref();
        let response = proto::CalculationResponse {
            result: input.a + input.b,
        };

        Ok(tonic::Response::new(response))
    }

    async fn provTasks(
        &self,
        request: tonic::Request<proto::UserRequest>,
    ) -> Result<tonic::Response<proto::UserResponse>, tonic::Status> {
        let input = request.get_ref();

        if input.b == 0 {
            return Err(tonic::Status::invalid_argument("cannot divide by zero"));
        }

        let response = proto::UserResponse {
            result: input.a / input.b,
        };

        Ok(tonic::Response::new(response))
    }
}

#[cfg(test)]
mod tests {
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
    fn fileappend_test() -> Result<(), Box<dyn std::error::Error>> {
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
}
