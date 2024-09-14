use crate::config::AppConfig;
use crate::datapolars;
use crate::error::CliError;
use crate::httprequests;

use crate::config;
use futures::future::ok;
use polars::functions::concat_df_horizontal;
use polars::prelude::CsvReader;

use std::thread;
use strum_macros::Display;
//use config::{AppConfig, ConfigError};
use crate::datapolars::pl_vstr_to_selects;
use polars::prelude::*;
use proto::user_server::User;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, OpenOptions},
    path::Path,
    time::Duration,
};

use tracing::{debug, info, warn};
pub mod proto {
    tonic::include_proto!("requestsautomation");

    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("user_descriptor");
}

#[derive(Clone, Copy, Display)]
// If we don't care about inner capitals, we don't need to set `serialize_all`
// and can leave parenthesis empty.
#[strum(serialize_all = "lowercase")]
enum Action {
    Retry,
    ManualComplete,
}

impl TryFrom<i32> for Action {
    type Error = ();
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            x if x == Action::Retry as i32 => Ok(Action::Retry),
            x if x == Action::ManualComplete as i32 => Ok(Action::ManualComplete),
            _ => Err(()),
        }
    }
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

/* impl From<Resp> for proto::Dictionary {
    fn from(item: Resp) -> Self {
        todo!()
    }
} */

fn action_mapper(re: tonic::Request<proto::ProvAcionRequest>) -> Result<String, CliError> {
    let action = match &re.get_ref().action.try_into() {
        Ok(Action::Retry) => Some(Action::Retry.to_string()),
        Ok(Action::ManualComplete) => Some(Action::ManualComplete.to_string()),
        Err(_) => {
            panic!("Unknown action");
        }
    }
    .ok_or(CliError::EntityNotFound { entity: "", id: 1 })?;
    Ok(action)
}

#[tonic::async_trait]
impl User for UserService {
    async fn conf_reload(
        &self,
        _request: tonic::Request<proto::UserRequest>,
    ) -> Result<tonic::Response<proto::ConfigResponse>, tonic::Status> {
        //CONFIG data from file
        let file = "Config.toml";
        let conf = config::confload(file)
            .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;
        let mut confs = self.state.write().await;
        //CONFIG overwrite pointer
        *confs = conf;

        Ok(tonic::Response::new(proto::ConfigResponse {
            result: "Reload successful".to_string(),
        }))
    }
    //TODO user to csv
    async fn gen_list(
        &self,
        _request: tonic::Request<proto::UserRequest>,
    ) -> Result<tonic::Response<proto::UserResponse>, tonic::Status> {
        //TODO Print list to CSV
        Ok(tonic::Response::new(proto::UserResponse { result: 1 }))
    }
    async fn db_delete(
        &self,
        _request: tonic::Request<proto::UserRequest>,
    ) -> Result<tonic::Response<proto::UserResponse>, tonic::Status> {
        let file = "path.csv";
        fs::remove_file(file)?;
        info!("File deleted");
        Ok(tonic::Response::new(proto::UserResponse { result: 2 }))
    }

    //TODO write to CSV
    async fn prov_tasks_list(
        &self,
        request: tonic::Request<proto::UserRequest>,
    ) -> Result<tonic::Response<proto::ListResponse>, tonic::Status> {
        //CONFIG data
        let conf = self.state.read().await;
        //HTTP Client create
        let client = reqwest::Client::new();
        //URL create
        let geturl = format!("{}{}{}", &conf.baseurl, conf.urlput, conf.urlget);
        let urllist = httprequests::urlsbuilder(&conf.baseurl, &conf.urlfilter);
        debug!("URLBUILDER: {:?}", &urllist);

        //URL loop
        for buildurl in urllist {
            //URL + arguments
            let newurl = format!("{}{}", geturl, buildurl);
            //DATA get from rest api
            let data = httprequests::get_data(
                &client,
                &newurl,
                &conf.username,
                &conf.password,
                conf.entries,
            )
            .await
            .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;

            //HEADER data extract
            let mut hm: HashMap<String, Series> =
                datapolars::getheaders(&client, &geturl, &conf.username, &conf.password)
                    .await
                    .map_err(|e| {
                        tonic::Status::new(tonic::Code::ResourceExhausted, format!("{:?}", e))
                    })?;
            //FILL series
            let data = datapolars::fillseries(data, &mut hm).clone();

            //DATAFRAME create
            let mut df_append = DataFrame::default();
            for (_i, v) in data {
                let df = v.into_frame();
                df_append = concat_df_horizontal(&[df_append, df])
                    .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;
            }

            //HEADER for select
            let df_header = vec![
                "Process Instance.Task Information.Creation Date",
                "Objects.Name",
                "Process Instance.Task Details.Key",
                "Process Definition.Tasks.Task Name",
                "Process Instance.Task Information.Target User",
            ];

            //SELECT columns
            let df = pl_vstr_to_selects(df_append, df_header)
                .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;
            let mut out = datapolars::get_data(df, &conf.filter1, &conf.filter2)
                .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;

            //List for count
            //let tasks = out["Process Instance.Task Details.Key"].as_list();
            //let _ids = out["Process Instance.Task Information.Target User"].as_list();

            //CSV write
            let fileexists = !Path::new("path.csv").exists();
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open("path.csv")
                .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;

            //CSV write header
            CsvWriter::new(&mut file)
                .include_header(fileexists)
                .finish(&mut out)
                .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;

            //DATAFRAME collect
            let mut dfa = out
                .clone()
                .lazy()
                .select([col("Process Instance.Task Information.Target User")])
                .collect()
                .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;

            //CSV write
            let mut file = std::fs::File::create("ids.csv")
                .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;

            //CSV write
            CsvWriter::new(&mut file)
                .finish(&mut dfa)
                .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;

            //CSV read
            let contents =
                fs::read_to_string("ids.csv").expect("Should have been able to read the file");

            //CSV read
            let splitted: Vec<&str> = contents.split('\n').collect();
            info!("Ids: {:?}", splitted);
            //new data from rest api
        }

        Ok(tonic::Response::new(proto::ListResponse {
            result: vec!["test".to_string()],
        }))
    }

    async fn prov_action(
        &self,
        request: tonic::Request<proto::ProvAcionRequest>,
    ) -> Result<tonic::Response<proto::Dictionary>, tonic::Status> {
        //CONFIG data from state
        let conf = self.state.read().await;

        //MATCH action with enum Protobuf to ENUM
        let action = action_mapper(request)?;

        //LOAD data from CSV
        let taskstosubmit = CsvReader::from_path(conf.filelist.clone())
            .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?
            .finish()
            .unwrap()["Process Instance.Task Details.Key"]
            .as_list();

        //let mut tasksdone: Vec<Resp> = vec![];
        //info!("Taskstosubmit: {:?}", taskstosubmit);

        //LOOP setup
        let client = reqwest::Client::new();
        let mut book_reviews: HashMap<String, proto::Task> = HashMap::new();
        for i in &taskstosubmit {
            //TODO Change to endpoint list of tasks
            if conf.checkmode {
                break;
            }

            //EXTRACT data from file
            let o = i
                .ok_or(CliError::EntityNotFound { entity: "", id: 0 })
                .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;
            let id = o.get(0).unwrap();

            let puturl = format!("{}{}{}{}", conf.baseurl, conf.urlput, "/", id);
            info!("Id: {id} PutUrl: {puturl}");

            //LOOP setup
            let status: u16 = 0;
            let mut retry: i32 = 0;
            while retry < 3 && status != 200 {
                retry += 1;

                let resp_result: Result<(), CliError> = match httprequests::retrycall(
                    &client,
                    &puturl.clone(),
                    action.clone(),
                    &conf.username.clone(),
                    &conf.password.clone(),
                )
                .await
                {
                    Ok(response) => {
                        info!("Status: {}", response.status().as_u16());
                        let newresp = proto::Task {
                            /* links: vec![Link {
                                rel: "self".to_string(),
                                href: puturl.clone(),
                            }], */
                            id: id.to_string(),
                            status: response.status().to_string(),
                        };
                        //tasksdone.push(newresp);
                        book_reviews.insert(id.to_string(), newresp);
                        Ok(())
                    }
                    Err(e) => {
                        warn!("Retry failed: {e:?}");
                        let newresp = proto::Task {
                            /* links: vec![Link {
                                rel: "self".to_string(),
                                href: puturl.clone(),
                            }], */
                            id: id.to_string(),
                            status: 400.to_string(),
                        };
                        //tasksdone.push(newresp);
                        book_reviews.insert(id.to_string(), newresp);
                        thread::sleep(Duration::from_secs(1));
                        Err(e)
                    }
                };
                if resp_result.is_err() {
                    warn!("Failed for: {:?}", resp_result);
                }
            }

            let df = CsvReader::from_path("path.csv").unwrap().finish().unwrap();
            let length = df["Process Instance.Task Details.Key"].len() as u32;

            if status == 200 {
                let mut df_a = df
                    .clone()
                    .lazy()
                    .slice(1, length - 1)
                    .collect()
                    .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;
                let mut file = std::fs::File::create("path.csv")
                    .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;
                CsvWriter::new(&mut file).finish(&mut df_a).unwrap();
                //retry += 1;
            }

            thread::sleep(Duration::from_secs(1));
        }
        let response = proto::Dictionary {
            pairs: book_reviews,
        };
        Ok(tonic::Response::new(response))
    }
}

/* mod tests {

    use super::*;

    #[test]
    fn urlsbuilder_test() -> Result<(), Box<dyn std::error::Error>> {
        todo!();
    }
} */
