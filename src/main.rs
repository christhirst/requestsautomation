mod config;
mod datapolars;
mod httprequests;

use axum::body;
use config::ConfigError;
use polars::functions::concat_df_horizontal;
use polars::prelude::*;

//polars::prelude::NamedFrom<std::vec::Vec<serde_json::Value>
use reqwest::{self, header::CONTENT_TYPE, Client, Error as rError, Response};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashMap,
    fmt,
    fs::{self, OpenOptions},
    thread,
    time::Duration,
    vec,
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

fn fillseries(data: Vec<Task>, hm: &mut HashMap<String, Series>) -> &mut HashMap<String, Series> {
    for i in data {
        for iii in &i.fields {
            let a = iii.name.clone();
            match iii.value.clone() {
                Value::Number(v) => {
                    let s = Series::new(&a, vec![v.to_string()]);
                    let oo = hm.get_mut(&a);
                    if let Some(x) = oo {
                        let _ = x.append(&s);
                    }
                }
                Value::String(v) => {
                    let s = Series::new(&a, vec![v.as_str()]);
                    let oo = hm.get_mut(&a);
                    if let Some(x) = oo {
                        let _ = x.append(&s);
                    }
                }
                _ => panic!("Type is wrong in value:Value matching"),
            };
        }
    }
    hm
}

async fn retrycall(
    client: &Client,
    url: String,
    body: String,
    username: String,
    password: String,
) -> Result<Response, CliError> {
    let response = client
        .put(url)
        .body(body)
        .header(CONTENT_TYPE, "application/json")
        .header("X-Requested-By", "rust")
        .basic_auth(username, Some(password))
        .timeout(Duration::from_secs(3))
        .send()
        .await?;
    Ok(response)
}

async fn getheaders(
    client: &Client,
    url: &str,
    username: &str,
    password: &str,
) -> Result<HashMap<String, Series>, CliError> {
    info!("headers: {:?}", url);
    let data = httprequests::get_data(client, url, username, password, 1).await?;
    let mut headers = vec!["".to_owned()];

    if let Some(ii) = data.clone().into_iter().next() {
        for iii in ii.fields {
            headers.push(iii.name.to_owned())
        }
    };
    let mut hm: HashMap<String, Series> = HashMap::from([]);
    info!("headers: {:?}", headers);

    for header in headers {
        let v1: Vec<String> = vec![];
        let series = Series::new(header.as_str(), v1);
        hm.entry(header).or_insert(series);
    }

    Ok(hm)
}

async fn urlbuilder(urlsnippets: &[&str], urlfilter: &Vec<String>) -> String {
    let mut url: String = "".to_string();
    for i in urlsnippets {
        url = format!("{}{}", url, i);
    }
    for i in urlfilter {
        url = format!("{}{}", url, i);
    }
    url
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
    let urlfilter = conf.urlfilter;
    let username = conf.username;
    let password = conf.password;
    let entries = conf.entries;
    let filter1 = conf.filter1;
    let filter2 = conf.filter2;
    let checkmode = conf.checkmode;
    let printmode = conf.printmode;
    let filemode = conf.filemode;

    info!("Version: {:?}", "v0.0.14");

    let geturl = format!("{}{}{}", url, urlput, urlget);
    let urllst: Vec<&str> = vec![&url, &urlput, &urlget];
    let client = reqwest::Client::new();
    let json_data = r#"{"action": "retry"}"#;
    let mut hm = getheaders(&client, &geturl, &username, &password).await?;

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
            println!("{}", puturl);

            let mut status: u16 = 0;
            let mut json: Resp;
            while retry < 0 || status != 200 {
                retry += 1;
                match retrycall(
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
                        print!("{e:?}");
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
        for i in urlfilter {
            let oo = urlbuilder(&urllst, &i);
            let data =
                httprequests::get_data(&client, &geturl, &username, &password, entries).await?;
            let hm = fillseries(data, &mut hm).clone();

            let mut df2 = DataFrame::default();
            for (_i, v) in hm {
                let df = v.into_frame();
                df2 = concat_df_horizontal(&[df2.clone(), df.clone()])?;
            }
            println!("{:?}", df2);

            let ss = vec![
                "Process Instance.Task Information.Creation Date",
                "Objects.Name",
                "Process Instance.Task Details.Key",
                "Process Definition.Tasks.Task Name",
                "Process Instance.Task Information.Target User",
            ];
            let df = pl_vstr_to_selects(df2, ss)?;

            println!("{:?}", df);

            let mut out = datapolars::get_data(df, &filter1, &filter2)?;
            println!("{:?}", out);

            let tasks = out["Process Instance.Task Details.Key"].as_list();
            let ids = out["Process Instance.Task Information.Target User"].as_list();

            /* let app = Router::new()
                // `POST /users` goes to `create_user`
                .route("/users", post(search_handler));

            // run our app with hyper, listening globally on port 3000
            let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
            axum::serve(listener, app).await.unwrap();*/

            if printmode {
                let mut file = std::fs::File::create("path.csv").unwrap();
                CsvWriter::new(&mut file).finish(&mut out).unwrap();

                let mut dfa = out
                    .clone()
                    .lazy()
                    .select([col("Process Instance.Task Information.Target User")])
                    .collect()?;

                let file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open("path.csv")
                    .unwrap();
                csv::Writer::from_writer(file);

                let mut file = std::fs::File::create("ids.csv").unwrap();
                CsvWriter::new(&mut file).finish(&mut dfa).unwrap();
                println!("{:?}", dfa);
                println!("{:?}", ids);

                let contents =
                    fs::read_to_string("ids.csv").expect("Should have been able to read the file");
                let splitted: Vec<&str> = contents.split("\n").collect();
                println!("{:?}", splitted);
            } else {
                let mut tasksdone: Vec<Resp> = vec![];
                for i in &tasks {
                    if checkmode {
                        break;
                    }
                    let o = i.ok_or(CliError::EntityNotFound { entity: "", id: 1 })?;
                    let id = o.get(0).unwrap();
                    info!("{}", id);

                    let puturl = format!("{}{}{}{}", url, urlput, "/", id);
                    println!("{}", puturl);

                    let response = retrycall(
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
                info!("{:?}", tasksdone);
            }
        }
    }
    Ok(())
}
