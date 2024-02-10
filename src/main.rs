mod config;

use config::{load_or_initialize, AppConfig, ConfigError};
use futures::io::Cursor;
use polars::functions::concat_df_horizontal;
use polars::prelude::*;
use polars::{
    chunked_array::ops::SortOptions,
    datatypes::DataType,
    df,
    lazy::{dsl::col, frame::IntoLazy},
    prelude::{JsonReader, Schema, SerReader},
};
use reqwest::Response;
use std::collections::HashMap;
use std::{error::Error, io::Read};

use reqwest::{
    self,
    header::{VacantEntry, ACCEPT, AUTHORIZATION, CONTENT_TYPE},
    Client, Error as rError,
};
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct action {
    pub action: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct resp {
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
    pub value: String,
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

fn calculate_length(mut s: Series, a: &Series) -> usize {
    s.append(a);
    todo!()
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
    println!("{:?}", file);
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

    return Ok(config);
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
        .header(AUTHORIZATION, "Bearer [AUTH_TOKEN]")
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json")
        .basic_auth(username, Some(password))
        .send()
        .await?;
    let json: Root = response.json().await?;
    Ok(json)
}

async fn getData(url: &str, username: &str, password: &str) -> Result<Vec<Task>, CliError> {
    let client = reqwest::Client::new();
    let mut data = fetchdata(&client, url, username, password).await?;

    let mut alltasks: Vec<Task> = vec![];

    let mut count = 0;

    let link = data.links.get(3);
    let next = link.ok_or(CliError::EntityNotFound { entity: "", id: 1 })?;
    let mut uri = "".to_owned();
    while data.has_more && count < 3 && next.rel == "next" {
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
    let url = conf.url;
    let username = conf.username;
    let password = conf.password;
    let checkmode = conf.checkmode;

    //let url = "http://localhost:8000/users";
    let data = getData(&url, &username, &password).await?;

    let mut header = vec!["".to_owned()];
    //let s1 = Series::new("Ocean", &["Atlantic", "Indian"]);
    if let Some(ii) = data.clone().into_iter().next() {
        let v: Vec<String> = vec![];
        for iii in ii.fields {
            header.push(iii.name.to_owned())
        }
        println!("{:?}", header)
    };

    let mut hm: HashMap<String, Series> = HashMap::from([]);

    for i in header {
        let mut v1: Vec<String> = vec![];
        let s = Series::new(i.as_str(), v1);
        hm.entry(i).or_insert(s);
    }

    for i in data {
        let mut v: Vec<String> = vec![];
        v.push("value".to_owned());
        for iii in &i.fields {
            let a = iii.name.clone();
            let s = Series::new(&a, vec![iii.value.clone()]);
            let oo = hm.get_mut(&a);
            if let Some(x) = oo {
                x.append(&s);
            }
        }
    }

    let mut df2 = DataFrame::default();
    for (i, v) in hm {
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
        .with_columns(
            [
                col("Process Instance.Task Information.Creation Date").cast(DataType::Datetime(
                    datatypes::TimeUnit::Milliseconds,
                    Some("UTC".to_owned()),
                )),
            ],
        )
        .with_columns([col("Process Instance.Task Details.Key").cast(DataType::Int64)])
        .sort(
            "Process Instance.Task Information.Creation Date",
            SortOptions {
                descending: true,
                nulls_last: true,
                ..Default::default()
            },
        )
        .collect()?;
    let ne = out.drop_in_place("");

    println!("{:?}", out);
    let tasks = out["Process Instance.Task Details.Key"].as_list();
    let json_data = r#"{"action": "retry"}"#;
    let client = reqwest::Client::new();
    for i in &tasks {
        let o = i.ok_or(CliError::EntityNotFound { entity: "", id: 1 })?;
        let id = o.get(0).unwrap();
        println!("{}", id);

        let action = action {
            action: "retry".to_owned(),
        };
        let mut owned_string: String = "http://localhost:3001/".to_owned();
        let mut uu = format!("{}{}", owned_string, id);
        println!("{uu}");
        let response = client
            .put(uu)
            .body(json_data.to_owned())
            .header(AUTHORIZATION, "Bearer [AUTH_TOKEN]")
            .header(CONTENT_TYPE, "application/json")
            .header(ACCEPT, "application/json")
            .basic_auth(username.clone(), Some(password.clone()))
            .send()
            .await?;
        let json: resp = response.json().await?;
        println!("{:?}", json);
        if checkmode {
            break;
        }
    }

    Ok(())
}