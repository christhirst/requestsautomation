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

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct action {
    pub action: String,
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
    let mut uri: String = url.to_owned();
    let mut count = 0;
    while data.has_more && count < 3 {
        data = fetchdata(&client, url, username, password).await?;
        alltasks.append(&mut data.tasks);
        if &data.links[3].rel == "next" {
            uri = data.links[3].rel.clone();
        }
        count += 1;
    }

    Ok(alltasks)
}

#[tokio::main]
async fn main() -> Result<(), CliError> {
    let file = "Config.toml";
    let conf = confload(file)?;
    let urls = conf.url;
    let username = conf.username;
    let password = conf.password;

    let url = "http://localhost:8000/users";
    let data = getData(url, &username, &password).await?;

    let mut header = vec!["".to_owned()];
    //let s1 = Series::new("Ocean", &["Atlantic", "Indian"]);
    if let Some(ii) = data.clone().into_iter().next() {
        let v: Vec<String> = vec![];
        for iii in ii.fields {
            header.push(iii.name.to_owned())
        }

        println!("{:?}", header)
    };
    //let mut v: Vec<Series> = vec![];
    //let mut s1 = Series::new("Ocean", v);
    let mut hm: HashMap<String, Series> = HashMap::from([]);

    for i in header {
        let mut v1: Vec<String> = vec![];
        let s = Series::new(i.as_str(), v1);
        hm.entry(i).or_insert(s);
    }

    /* let next = &response_json.links.get(3);
    println!("{:?}", next); */
    // for i in response_json.has_more {}

    for i in data {
        //df._add_columns(columns, schema)
        // let v: Vec<String> = vec![];

        let mut v: Vec<String> = vec![];
        v.push("value".to_owned());
        for iii in &i.fields {
            let a = iii.name.clone();
            let s = Series::new(&a, vec![iii.value.clone()]);
            let oo = hm.get_mut(&a);
            if let Some(x) = oo {
                x.append(&s);
            }
            //calculate_length(*oo, &s);
            //oo.append(todo!());

            //solar_distance.entry(ii.to_string()).or_insert(v.clone());
        }
        //solar_distance.entry(ii.name).or_insert(ii.value);
        /* v.push(i.fields[0].name);
        println!("{:?}", ii);
        println!("{:?}", ii.name); */
    }
    // let df1 = DataFrame::default();
    let mut df2 = DataFrame::default();
    for (i, v) in hm {
        //let p = v.str.to_datetime.unwrap();
        //print!("{:?}", p);
        let df = v.into_frame();
        df2 = concat_df_horizontal(&[df2.clone(), df.clone()])?;
    }

    let mut out = df2
        .clone()
        .lazy()
        .select([
            col("*"),
            col("APP_INSTANCE_NAME")
                .str()
                .contains(lit("CAccount"), false)
                .alias("instance"),
            col("Process Definition.Tasks.Task Name")
                .str()
                .contains(lit("Update"), false)
                .alias("update"),
        ])
        .filter(col("instance").eq(lit(true)))
        .filter(col("update").eq(lit(true)))
        .with_columns([
            //col("*"),
            col("Process Instance.Task Information.Creation Date").cast(DataType::Datetime(
                datatypes::TimeUnit::Milliseconds,
                Some("UTC".to_owned()),
            )),
        ])
        .sort(
            "Process Instance.Task Information.Creation Date",
            SortOptions {
                descending: true,
                nulls_last: true,
                ..Default::default()
            },
        )
        //.filter(col("Process Instance.Task Information.Creation Date").is_in(lit("Update")))
        .collect()?;
    let ne = out.drop_in_place("");

    //let filtered_df = out.lazy().filter(col("regex").eq(lit(true))).collect();

    println!("{:?}", df2);
    println!("{:?}", out);
    /*   let tasks = out["Process Instance.Task Details.Key"].as_list();

       for i in &tasks {
           println!("{:?}", i);
       }
    */
    /* client
    .put(url)
    .header("x-Requested-By", "Rust")
    .header("Content-Type", "application/json")
    .basic_auth("username", Some("password"))
    .send()
    .await?; */

    /* */

    //let response = reqwest::get("https://jsonplaceholder.typicode.com/todos/1").await?;
    //let mut body = std::io::Cursor::new(response.bytes().await?);
    //let mut buffer = Vec::new();
    //let ww = body.read_to_end(&mut buffer)?;

    /*  let df = JsonReader::new(body).finish()?;
       let out = df
           .clone()
           .lazy()
           .select([col("tasks")])
           .explode(["tasks"])
           .select([col("tasks")])
           .unnest(["tasks"])
           .select([col("fields")])
           /* .sort(
               "len",
               SortOptions {
                   descending: true,
                   nulls_last: true,
                   ..Default::default()
               },
           ) */
           .collect()?;
       for i in out.iter() {
           println!("{:?}", i)
       }

       //let out2 = out.get_row(0);
       println!("{:?}", out);
    */
    // read the response into df, per method of JsonReader

    //println!("{:?}", response);
    /*  match response.status() {
        reqwest::StatusCode::OK => {
            // on success, parse our JSON to an APIResponse
            match response.json::<APIResponse>().await {
                Ok(parsed) => println!("Success! {:?}", parsed),
                Err(_) => println!("Hm, the response didn't match the shape we expected."),
            };
        }
        reqwest::StatusCode::UNAUTHORIZED => {
            println!("Need to grab a new token");
        }
        _ => {
            panic!("Uh oh! Something unexpected happened.");
        }
    } */

    /*  println!("{:?}", df);

    println!("Hello, world!"); */
    todo!()
}
