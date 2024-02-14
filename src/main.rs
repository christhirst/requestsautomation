mod config;

use chrono::prelude::*;
use config::{load_or_initialize, AppConfig, ConfigError};
use polars::functions::concat_df_horizontal;
use polars::lazy::dsl::StrptimeOptions;
use polars::prelude::*;
use polars::{
    chunked_array::ops::SortOptions,
    datatypes::DataType,
    lazy::{dsl::col, frame::IntoLazy},
};

//polars::prelude::NamedFrom<std::vec::Vec<serde_json::Value>
use reqwest::{
    self,
    header::{ACCEPT, CONTENT_TYPE},
    Client, Error as rError,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

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

fn confload(file: &str) -> Result<AppConfig, CliError> {
    let config: AppConfig = match load_or_initialize(file) {
        Ok(v) => v,
        Err(err) => {
            /* match err {
                ConfigError::IoError(err) => {
                    eprintln!("An error occurred while loading the config: {err}");
                }
                ConfigError::InvalidConfig(err) => {
                    eprintln!("An error occurred while parsing the config:");
                    eprintln!("{err}");
                }
            } */
            return Err(err.into());
        }
    };

    Ok(config)
    //println!("{:?}", config);
}

async fn fetchdata(
    client: &Client,
    url: &str,
    username: &str,
    password: &str,
) -> Result<Root, CliError> {
    let response = client
        .get(url)
        //.header(AUTHORIZATION, "Bearer [AUTH_TOKEN]")
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json")
        .basic_auth(username, Some(password))
        .send()
        .await?;
    println!("{:?}", response);
    let json: Root = response.json().await?;
    Ok(json)
}

async fn get_data(url: &str, username: &str, password: &str) -> Result<Vec<Task>, CliError> {
    let client = reqwest::Client::new();
    let data = fetchdata(&client, url, username, password).await?;

    let mut alltasks: Vec<Task> = vec![];

    let mut count = 0;

    let link = data.links.get(3);
    let next = link.ok_or(CliError::EntityNotFound { entity: "", id: 1 })?;
    let mut uri = "".to_owned();
    while data.has_more && count < 1 && next.rel == "next" {
        if next.rel == "next" {
            uri = next.href.clone();
        }
        //data = fetchdata(&client, &ouri, username, password).await?.tasks;
        alltasks.append(&mut fetchdata(&client, &uri, username, password).await?.tasks);

        count += 1;
    }

    Ok(alltasks)
}

#[tokio::main]
async fn main() -> Result<(), CliError> {
    let file = "Config.toml";
    let conf = confload(file)?;
    let url = conf.baseurl;
    let urlget = conf.urlget;
    let urlput = conf.urlput;
    let username = conf.username;
    let password = conf.password;
    let checkmode = conf.checkmode;

    let geturl = format!("{}{}{}", url, urlput, urlget);
    let data = get_data(&geturl, &username, &password).await?;
    let mut header = vec!["".to_owned()];
    //let s1 = Series::new("Ocean", &["Atlantic", "Indian"]);
    if let Some(ii) = data.clone().into_iter().next() {
        for iii in ii.fields {
            header.push(iii.name.to_owned())
        }
    };

    let mut hm: HashMap<String, Series> = HashMap::from([]);

    for i in header {
        let v1: Vec<String> = vec![];
        let s = Series::new(i.as_str(), v1);
        hm.entry(i).or_insert(s);
    }

    for i in data {
        for iii in &i.fields {
            let a = iii.name.clone();
            let s = Series::new(&a, vec![iii.value.to_string().clone()]);
            let oo = hm.get_mut(&a);
            if let Some(x) = oo {
                let _ = x.append(&s);
            }
        }
    }

    let mut df2 = DataFrame::default();
    for (_i, v) in hm {
        let df = v.into_frame();
        df2 = concat_df_horizontal(&[df2.clone(), df.clone()])?;
    }
    println!("{:?}", df2);
    let mut out = df2
        .clone()
        .lazy()
        .filter(
            col("APP_INSTANCE_NAME")
                .str()
                .contains(lit("CAccount"), false),
        )
        .filter(
            col("Process Definition.Tasks.Task Name")
                .str()
                .contains(lit("Update"), false),
        )
        .with_columns([col("Process Instance.Task Information.Creation Date")
            .str()
            .strptime(
                DataType::Datetime(TimeUnit::Milliseconds, None),
                StrptimeOptions {
                    format: Some("%Y-%m-%dT%H:%M:%SZ".to_owned()),
                    strict: false,
                    exact: false,
                    cache: false,
                },
                lit("raise"),
            )])
        .with_columns([col("Process Instance.Task Details.Key").cast(DataType::Int64)])
        .sort(
            "Process Instance.Task Information.Creation Date",
            SortOptions {
                descending: false,
                nulls_last: true,
                ..Default::default()
            },
        )
        .collect()?;
    let _ne = out.drop_in_place("");

    println!("{:?}", out);
    let tasks = out["Process Instance.Task Details.Key"].as_list();
    let json_data = r#"{"action": "retry"}"#;
    let client = reqwest::Client::new();
    for i in &tasks {
        if checkmode {
            break;
        }

        let o = i.ok_or(CliError::EntityNotFound { entity: "", id: 1 })?;
        let id = o.get(0).unwrap();
        println!("{}", id);

        //let mut owned_string: String = "http://localhost:3001/provtasks/".to_owned();
        let puturl = format!("{}{}{}{}", url, urlput, "/", id);
        let response = client
            .put(puturl)
            .body(json_data.to_owned())
            //.header(AUTHORIZATION, "Bearer [AUTH_TOKEN]")
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json")
            .basic_auth(username.clone(), Some(password.clone()))
            .send()
            .await?;
        let json: Resp = response.json().await?;
        println!("{:?}", json);
    }

    Ok(())
}
