use crate::config::{AppConfig, ConfigError};
use crate::datapolars;
use crate::httprequests;
use crate::{config, CliError};
use polars::functions::concat_df_horizontal;
use polars::prelude::CsvReader;
use std::thread;
//use config::{AppConfig, ConfigError};
use crate::datapolars::pl_vstr_to_selects;
use polars::prelude::*;
use proto::user_server::{User, UserServer};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fmt,
    fs::{self, OpenOptions},
    path::Path,
    time::Duration,
};

use tracing::{debug, info};
pub mod proto {
    tonic::include_proto!("requestsautomation");

    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("user_descriptor");
}

type State = std::sync::Arc<tokio::sync::RwLock<AppConfig>>;

#[derive(Debug, Default)]
pub struct UserService {
    state: State,
    config: AppConfig,
}
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub rel: String,
    pub href: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resp {
    pub links: Vec<Link>,
    pub id: String,
    pub status: String,
}

#[tonic::async_trait]
impl User for UserService {
    async fn conf_reload(
        &self,
        request: tonic::Request<proto::UserRequest>,
    ) -> Result<tonic::Response<proto::ConfigResponse>, tonic::Status> {
        let file = "Config.toml";
        let conf = config::confload(file)
            .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;
        let mut confs = self.state.write().await;
        *confs = conf;

        let input = request.get_ref();
        let response = proto::ConfigResponse {
            result: "test".to_string(),
        };
        Ok(tonic::Response::new(response))
    }
    //TODO user to csv
    async fn list(
        &self,
        request: tonic::Request<proto::UserRequest>,
    ) -> Result<tonic::Response<proto::UserResponse>, tonic::Status> {
        let input = request.get_ref();
        let response = proto::UserResponse {
            result: input.a + input.b,
        };

        Ok(tonic::Response::new(response))
    }
    async fn db_delete(
        &self,
        request: tonic::Request<proto::UserRequest>,
    ) -> Result<tonic::Response<proto::UserResponse>, tonic::Status> {
        let input = request.get_ref();
        let response = proto::UserResponse {
            result: input.a + input.b,
        };
        let file = "Config.toml";
        fs::remove_file(file)?;
        Ok(tonic::Response::new(response))
    }

    //TODO write to CSV
    async fn prov_tasks(
        &self,
        request: tonic::Request<proto::UserRequest>,
    ) -> Result<tonic::Response<proto::ListResponse>, tonic::Status> {
        //Config data
        let conf = self.state.read().await;
        //Client create
        let client = reqwest::Client::new();
        //Url create
        let geturl = format!("{}{}{}", &conf.baseurl, conf.urlput, conf.urlget);
        let urllist = httprequests::urlsbuilder(&conf.baseurl, &conf.urlfilter);
        debug!("URLBUILDER: {:?}", &urllist);

        //Url loop
        for buildurl in urllist {
            let newurl = format!("{}{}", geturl, buildurl);
            //Rest api get data
            let data = httprequests::get_data(
                &client,
                &newurl,
                &conf.username,
                &conf.password,
                conf.entries,
            )
            .await
            .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;

            //get header data
            let mut hm: HashMap<String, Series> =
                datapolars::getheaders(&client, &geturl, &conf.username, &conf.password)
                    .await
                    .map_err(|e| {
                        tonic::Status::new(tonic::Code::ResourceExhausted, format!("{:?}", e))
                    })?;

            //fill series
            let data = datapolars::fillseries(data, &mut hm).clone();

            //create dataframe
            let mut df_append = DataFrame::default();
            for (_i, v) in data {
                let df = v.into_frame();
                df_append = concat_df_horizontal(&[df_append, df])
                    .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;
            }

            //Header for select
            let df_header = vec![
                "Process Instance.Task Information.Creation Date",
                "Objects.Name",
                "Process Instance.Task Details.Key",
                "Process Definition.Tasks.Task Name",
                "Process Instance.Task Information.Target User",
            ];

            //Columns select
            let df = pl_vstr_to_selects(df_append, df_header)
                .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;
            let mut out = datapolars::get_data(df, &conf.filter1, &conf.filter2)
                .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;

            //List for count
            let tasks = out["Process Instance.Task Details.Key"].as_list();
            //let _ids = out["Process Instance.Task Information.Target User"].as_list();

            //CSV write
            let fileexists = !Path::new("path.csv").exists();
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open("path.csv")
                .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;

            CsvWriter::new(&mut file)
                .include_header(fileexists)
                .finish(&mut out)
                .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;

            //file.write_all(b"\n").unwrap();

            let mut dfa = out
                .clone()
                .lazy()
                .select([col("Process Instance.Task Information.Target User")])
                .collect()
                .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;

            let mut file = std::fs::File::create("ids.csv")
                .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;
            CsvWriter::new(&mut file)
                .finish(&mut dfa)
                .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;

            let contents =
                fs::read_to_string("ids.csv").expect("Should have been able to read the file");
            let splitted: Vec<&str> = contents.split('\n').collect();

            info!("Ids: {:?}", splitted);

            //new data from rest api
        }
        let input = request.get_ref();
        if input.b == 0 {
            return Err(tonic::Status::invalid_argument("cannot divide by zero"));
        }

        let response = proto::ListResponse {
            result: vec!["test".to_string()],
        };
        Ok(tonic::Response::new(response))
    }
}
