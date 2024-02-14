mod config;
mod datapolars;
mod httprequests;

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

/* fn confload(file: &str) -> Result<AppConfig, CliError> {
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
} */

fn fillseries(data: Vec<Task>, hm: &mut HashMap<String, Series>) -> &mut HashMap<String, Series> {
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
    hm
}

#[tokio::main]
async fn main() -> Result<(), CliError> {
    let w: u64 = 1;
    let file = "Config.toml";
    let conf = config::confload(file)?;
    let url = conf.baseurl;
    let urlget = conf.urlget;
    let urlput = conf.urlput;
    let username = conf.username;
    let password = conf.password;
    let checkmode = conf.checkmode;

    let geturl = format!("{}{}{}", url, urlput, urlget);
    let data = httprequests::get_data(&geturl, &username, &password).await?;
    let mut headers = vec!["".to_owned()];

    if let Some(ii) = data.clone().into_iter().next() {
        for iii in ii.fields {
            headers.push(iii.name.to_owned())
        }
    };

    let mut hm: HashMap<String, Series> = HashMap::from([]);

    for header in headers {
        let v1: Vec<String> = vec![];
        let series = Series::new(header.as_str(), v1);
        hm.entry(header).or_insert(series);
    }

    let hm = fillseries(data, &mut hm).clone();

    let mut df2 = DataFrame::default();
    for (_i, v) in hm {
        let df = v.into_frame();
        df2 = concat_df_horizontal(&[df2.clone(), df.clone()])?;
    }
    println!("{:?}", df2);

    let mut out = datapolars::get_data(df2)?;
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
