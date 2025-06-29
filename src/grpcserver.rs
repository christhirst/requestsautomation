use crate::config::Database;
//use crate::config3::AppConfig;
use crate::db::types::Task;
use crate::error::CliError;
use crate::httprequests;

use crate::Settings;
use crate::{datapolars, grpcserver};

use chrono::DateTime;
use chrono::Utc;
//use crate::config3;
use polars::functions::concat_df_horizontal;
use polars::prelude::CsvReader;
use prost_types::Timestamp;
use surrealdb::engine::remote::ws::Client;
use surrealdb::Surreal;
use tokio::sync::Mutex;

use std::thread;
use strum_macros::Display;
//use config::{AppConfig, ConfigError};
use crate::datapolars::pl_vstr_to_selects;
use polars::prelude::*;
use proto::user_server::User;
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
type State = std::sync::Arc<tokio::sync::RwLock<Option<Settings>>>;
//type DbService = std::sync::Arc<tokio::sync::RwLock<Option<DBService>>>;

#[derive(Debug)]
pub struct DBService {
    pub conf: Database,
    pub db: Option<Surreal<Client>>,
    pub jwt: Option<String>,
}

#[derive(Debug)]
pub struct UserService {
    pub state: State,
    //pub(crate) config: Option<AppConfig>,
    pub db: Option<Arc<Mutex<DBService>>>,
}

impl UserService {
    pub async fn new() -> Result<Self, surrealdb::Error> {
        info!("Loading configuration");
        let conf = Settings::new().unwrap();
        let db_conf = conf.database.clone();

        let mut db = None;
        if conf.db {
            info!("DB enabled");
            let dbs = DBService::new(db_conf).await?;
            let shared_db_service = Arc::new(Mutex::new(dbs));
            db = Some(shared_db_service);
        } else {
            info!("DB disabled");
        }
        let state = Arc::new(tokio::sync::RwLock::new(Some(conf)));
        let calc = grpcserver::UserService {
            state: state,
            db: db,
        };

        Ok(calc)
    }
}

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
        //TODO DB reload
        let file = "Config.toml";
        /* let conf = config3::confload(file)
        .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?; */

        let conf = Settings::new().unwrap();
        let mut confs = self.state.write().await;
        //CONFIG overwrite pointer
        *confs = Some(conf);

        Ok(tonic::Response::new(proto::ConfigResponse {
            result: "Reload successful".to_string(),
        }))
    }
    //TODO user to csv
    async fn gen_list(
        &self,
        _request: tonic::Request<proto::UserRequest>,
    ) -> Result<tonic::Response<proto::ListResponse>, tonic::Status> {
        let mut df = CsvReader::from_path("path.csv").unwrap().finish().unwrap();

        //TODO Print list to CSV + Database

        // Convert chrono::DateTime<Utc> to prost_types::Timestamp
        let now = Utc::now();
        let timestamp = Timestamp {
            seconds: now.timestamp(),
            nanos: now.timestamp_subsec_nanos() as i32,
        };

        Ok(tonic::Response::new(proto::ListResponse {
            result: vec!["1".to_string()],
            time: Some(timestamp),
        }))
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
        let conf = conf.as_ref().ok_or_else(|| {
            tonic::Status::new(
                tonic::Code::NotFound,
                "Configuration not found, please reload",
            )
        })?;
        let conf = &conf.grpc;
        let timeout = conf.timeout;
        //HTTP Client create
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout))
            .build()
            .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{:?}", e)))?;

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

            if self.db.is_none() {
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
            } else {
                for idx in 0..out.height() {
                    let row = out.get_row(idx).unwrap().0;

                    //Convert DateTime to DateTime<Utc>
                    let s = format!("{}Z", row[0]);
                    let date = s.parse::<DateTime<Utc>>().unwrap();

                    let task_row = Task {
                        id: None,
                        process_definition_tasks_task_name: row[3].to_string(),
                        process_instance_task_information_target_user: row[4].to_string(),
                        process_instance_task_details_key: row[2].to_string(),
                        objects_name: row[1].to_string(),
                        process_instance_task_information_creation_date: date,
                    };
                    let mut db = self.db.as_ref().unwrap().lock().await;
                    db.db_create_entry("task", task_row.clone())
                        .await
                        .unwrap_or_else(|e| {
                            warn!("Failed to create entry: {:?}", e);
                            panic!("Failed to create entry: {:?}", e);
                        });

                    /*  &self
                    .db
                    .as_ref()
                    .unwrap()
                    .db_create_entry("task", task_row.clone())
                    .await
                    .unwrap_or_else(|e| {
                        warn!("Failed to create entry: {:?}", e);
                        panic!("Failed to create entry: {:?}", e);
                    }); */

                    println!("{:?}", task_row);

                    println!("{:?}", row);
                }

                /*  dfa.iter().for_each(|s| {
                                   info!("Target User: {:?}", s);
                               });
                */
                /* dfa.iter().for_each(|s| {
                    info!(
                        "Target User: {:?}",
                        s.iter().map(|v| v.to_string()).collect::<Vec<String>>()
                    );
                }); */
            }
            //new data from rest api
        }
        let now = Utc::now();
        let timestamp = Timestamp {
            seconds: now.timestamp(),
            nanos: now.timestamp_subsec_nanos() as i32,
        };

        Ok(tonic::Response::new(proto::ListResponse {
            result: vec!["test".to_string()],
            time: Some(timestamp),
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
        let db_mod = conf.as_ref().map_or(false, |c| c.db);
        let conf = &conf
            .as_ref()
            .ok_or_else(|| {
                tonic::Status::new(
                    tonic::Code::NotFound,
                    "Configuration not found, please reload",
                )
            })?
            .grpc;
        let path = conf.filelist.clone();

        if self.db.is_none() {
            //LOAD data from CSV
            let taskstosubmit = CsvReader::from_path(&path)
                .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?
                .finish()
                .unwrap()["Process Instance.Task Details.Key"]
                .as_list();

            //CLIENT SETUP
            let client = reqwest::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(conf.timeout))
                .timeout(std::time::Duration::from_secs(conf.timeout))
                .build()
                .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{:?}", e)))?;

            //LOOP setup
            //LIST of retried tasks
            let mut tasks_retried: HashMap<String, proto::Task> = HashMap::new();
            //LIST interate
            for i in &taskstosubmit {
                //TODO Change to endpoint list of tasks
                if conf.checkmode {
                    break;
                }

                //EXTRACT data from Struct
                let o = i
                    .ok_or(CliError::EntityNotFound { entity: "", id: 0 })
                    .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;
                //LIST get first value
                let id = o
                    .get(0)
                    .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;

                //URL generate PUT
                let puturl = format!("{}{}{}{}", conf.baseurl, conf.urlput, "/", id);
                info!("Id: {id} PutUrl: {puturl}");

                //LOOP setup
                let status: u16 = 0;
                let mut retry: i32 = 0;
                //RETRY Test Config
                while retry < 3 && status != 200 {
                    retry += 1;
                    info!("retry: {retry} ");
                    //TODO extract to function
                    //HTTP CALL
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
                            thread::sleep(Duration::from_secs(1));
                            //tasksdone.push(newresp);
                            tasks_retried.insert(id.to_string(), newresp);
                            Ok(())
                        }
                        Err(e) => {
                            warn!("Retry failed: {e:?}");
                            let newresp = proto::Task {
                                id: id.to_string(),
                                status: 400.to_string(),
                            };
                            //tasksdone.push(newresp);
                            tasks_retried.insert(id.to_string(), newresp);
                            thread::sleep(Duration::from_secs(1));
                            Err(e)
                        }
                    };
                    if resp_result.is_err() {
                        warn!("Failed for: {:?}", resp_result);
                    }
                }
                if !db_mod {
                    //LIST POP First
                    let df = CsvReader::from_path(&path).unwrap().finish().unwrap();
                    let length = df["Process Instance.Task Details.Key"].len() as u32;
                    let mut df_a =
                        df.clone()
                            .lazy()
                            .slice(1, length - 1)
                            .collect()
                            .map_err(|e| {
                                tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e))
                            })?;
                    let mut file = std::fs::File::create(&path).map_err(|e| {
                        tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e))
                    })?;
                    CsvWriter::new(&mut file).finish(&mut df_a).unwrap();
                } else {
                    let mut db: tokio::sync::MutexGuard<'_, DBService> =
                        self.db.as_ref().unwrap().lock().await;
                    let row = db.db_get_first_row("task").await.map_err(|e| {
                        tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e))
                    })?;

                    //let db = self.db_delete("task").await;
                    //.unwrap_or_else(|| create_db_client());

                    //DATABASE task delete
                }

                thread::sleep(Duration::from_millis(conf.sleep));
            }
            let response = proto::Dictionary {
                pairs: tasks_retried,
            };
            Ok(tonic::Response::new(response))
        } else {
            //SELECT FROM DB
            while true {
                let mut db = self.db.as_ref().unwrap().lock().await;
                let ii = db
                    .db_get_first_row("task")
                    .await
                    .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;
                //Process Instance.Task Details.Key
                let task_id = ii.process_instance_task_details_key.clone();
                let entry_id = ii.id.unwrap();

                db.db_delete_by_id(entry_id)
                    .await
                    .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;
                thread::sleep(Duration::from_millis(conf.sleep));
            }
            Ok(tonic::Response::new(todo!()))
        }
    }
}

mod tests {

    /* #[test]
    fn urlsbuilder_test() {} */
}
