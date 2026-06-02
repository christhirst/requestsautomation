use crate::config::Database;
use crate::dataendpoint::action_mapper;
//use crate::config3::AppConfig;
use crate::db::types::Task;
use crate::error::CliError;

use crate::Settings;
use crate::file::{data_load, file_header};
use crate::http::client::rest_client;
use crate::http::httprequests::{self, filterbuilder};
use crate::{datapolars, grpcserver};

use chrono::DateTime;
use chrono::Utc;
//use crate::config3;
use polars::functions::concat_df_horizontal;
use polars::prelude::CsvReader;
use prost_types::Timestamp;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Client;
use tokio::sync::Mutex;

use std::thread;

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

type State = std::sync::Arc<tokio::sync::RwLock<Option<Settings>>>;

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
    async fn get_config(&self) -> Result<Settings, CliError> {
        debug!("Config get");
        let guard = self.state.read().await;
        let settings = guard.as_ref().ok_or(CliError::EntityNotFound {
            entity: "Task",
            id: 42,
        })?;
        Ok(settings.clone())
    }
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

async fn perform_task_retry(
    client: &reqwest::Client,
    conf: &crate::config::AppConfig,
    id: &str,
    action: crate::types::ProvAcionRequest,
    tasks_retried: &mut std::collections::HashMap<String, proto::Task>,
) {
    //URL generate PUT
    let puturl = format!("{}{}{}{}", conf.baseurl, conf.urlput, "/", id);
    debug!("Id: {id} PutUrl: {puturl}");

    //LOOP setup
    let mut status: u16 = 0;
    let mut retry: i32 = 0;
    //RETRY Test Config
    while retry < 3 && status != 200 {
        retry += 1;
        info!("retry: {retry} ");
        //HTTP CALL
        let resp_result: Result<(), CliError> = match httprequests::retrycall(
            client,
            &puturl.clone(),
            action.clone(),
            &conf.username.clone(),
            &conf.password.clone(),
        )
        .await
        {
            Ok(response) => {
                info!("Status: {}", response.status().as_u16());
                status = response.status().as_u16();
                let newresp = proto::Task {
                    id: id.to_string(),
                    status: response.status().to_string(),
                };
                thread::sleep(Duration::from_secs(1));
                tasks_retried.insert(id.to_string(), newresp);
                Ok(())
            }
            Err(e) => {
                warn!("Retry failed: {e:?}");
                let newresp = proto::Task {
                    id: id.to_string(),
                    status: 400.to_string(),
                };
                tasks_retried.insert(id.to_string(), newresp);
                thread::sleep(Duration::from_secs(1));
                Err(e)
            }
        };
        if resp_result.is_err() {
            warn!("Failed for: {:?}", resp_result);
        }
    }
}

#[tonic::async_trait]
impl User for UserService {
    async fn conf_reload(
        &self,
        _request: tonic::Request<proto::UserRequest>,
    ) -> Result<tonic::Response<proto::ConfigResponse>, tonic::Status> {
        //CONFIG data from file
        let conf = match Settings::new() {
            Ok(c) => c,
            Err(e) => {
                return Err(tonic::Status::new(
                    tonic::Code::Internal,
                    format!("Failed to load configuration: {:?}", e),
                ));
            }
        };

        // Try to reload DB if enabled
        let mut db_status = "Database disabled".to_string();
        if conf.db {
            if let Some(db_service) = &self.db {
                let mut db = db_service.lock().await;
                match db.db_reload().await {
                    Ok(_) => {
                        db_status = "Database connection reloaded successfully".to_string();
                    }
                    Err(e) => {
                        return Err(tonic::Status::new(
                            tonic::Code::Internal,
                            format!("Configuration reloaded, but database reload failed: {:?}", e),
                        ));
                    }
                }
            } else {
                db_status = "Database enabled in config, but database service not initialized".to_string();
            }
        }

        let mut confs = self.state.write().await;
        //CONFIG overwrite pointer
        *confs = Some(conf);

        Ok(tonic::Response::new(proto::ConfigResponse {
            result: format!("Reload successful. Status: {}", db_status),
        }))
    }
    async fn check_con(
        &self,
        _request: tonic::Request<proto::UserRequest>,
    ) -> Result<tonic::Response<proto::ConfigResponse>, tonic::Status> {
        let settings = self.get_config().await?;
        let url = format!("{}{}", settings.grpc.baseurl, settings.grpc.urlput);
        let client = reqwest::Client::new();
        let response = client.get(&url).send().await.map_err(|e| {
            tonic::Status::new(
                tonic::Code::Unavailable,
                format!("Connection failed to URL {}: {:?}", url, e),
            )
        })?;

        let status_code = response.status();
        let is_success = status_code.is_success();
        
        let body = response.text().await.unwrap_or_else(|e| {
            format!("Failed to read response body: {:?}", e)
        });

        let status_text = if is_success { "SUCCESS" } else { "FAILED" };
        let body_preview = if body.len() > 500 {
            format!("{}...", &body[..500])
        } else {
            body.clone()
        };

        let result_message = format!(
            "Connection Status: {}\nURL Checked: {}\nHTTP Status Code: {}\nResponse Body Preview:\n{}",
            status_text,
            url,
            status_code.as_u16(),
            body_preview
        );

        Ok(tonic::Response::new(proto::ConfigResponse { result: result_message }))
    }

    async fn gen_list(
        &self,
        request: tonic::Request<proto::FilterRequest>,
    ) -> Result<tonic::Response<proto::ListResponse>, tonic::Status> {
        self.prov_tasks_list(request).await
    }
    async fn db_delete(
        &self,
        _request: tonic::Request<proto::UserRequest>,
    ) -> Result<tonic::Response<proto::UserResponse>, tonic::Status> {
        let settings = self.get_config().await?;

        if settings.db {
            let db = self.db.as_ref().unwrap().lock().await;
            let ii = db.db_delete_all("task").await?;
            let ii = ii.len();

            let _conf_db = settings.database;

            return Ok(tonic::Response::new(proto::UserResponse {
                result: ii as i64,
            }));
        } else {
            let file = settings.grpc.filelist;
            fs::remove_file(file)?;
            info!("File deleted");
        }

        Ok(tonic::Response::new(proto::UserResponse { result: 2 }))
    }

    //TODO Print CSV
    async fn prov_tasks_list(
        &self,
        request: tonic::Request<proto::FilterRequest>,
    ) -> Result<tonic::Response<proto::ListResponse>, tonic::Status> {
        let req = request.into_inner();
        info!("Request: {:?}", req);
        
        //CONFIG data from file
        let settings = self.get_config().await?;
        let conf = settings.grpc;
        let timeout = conf.timeout;
        let path = &conf.filelist.clone();
        debug!("CONFIGDATA successful");

        let mode = req.mode.to_lowercase();
        let limit = req.limit;

        let is_db_mode = match mode.as_str() {
            "db" | "database" => {
                if self.db.is_none() {
                    return Err(tonic::Status::new(
                        tonic::Code::FailedPrecondition,
                        "Database is not enabled in settings",
                    ));
                }
                true
            }
            "csv" | "file" => false,
            "direct" | "now" | "return" => false,
            _ => self.db.is_some(),
        };

        let is_direct_mode = match mode.as_str() {
            "direct" | "now" | "return" => true,
            _ => false,
        };

        let urlget = if req.urlget.is_empty() {
            conf.urlget.clone().unwrap_or_default()
        } else {
            req.urlget
        };

        let urllist = filterbuilder(req.urlfilter);

        //HTTP Client create
        let client = rest_client(timeout)?;
        //URL create
        let geturl = format!("{}{}{}", &conf.baseurl, conf.urlput, urlget);
        debug!("URLBUILDER: {:?}", &urllist);

        let mut tasks_written = 0;
        let mut response_tasks = vec![];

        for buildurl in urllist {
            if limit > 0 && tasks_written >= limit {
                break;
            }

            //URL + arguments
            let newurl = if !buildurl.is_empty() {
                if geturl.contains("?q=") {
                    let trimmed = geturl.trim();
                    if trimmed.ends_with("AND") || trimmed.ends_with("AND ") || trimmed.ends_with("AND+") || trimmed.ends_with("+AND+") || trimmed.ends_with("+AND") {
                        format!("{}{}", trimmed, buildurl)
                    } else {
                        format!("{} AND {}", trimmed, buildurl)
                    }
                } else {
                    format!("{}?q={}", geturl, buildurl)
                }
            } else {
                if geturl.contains("?q=") {
                    let mut trimmed = geturl.trim().to_string();
                    if trimmed.ends_with("AND") {
                        trimmed = trimmed[..trimmed.len() - 3].trim().to_string();
                    } else if trimmed.ends_with("AND+") {
                        trimmed = trimmed[..trimmed.len() - 4].trim().to_string();
                    } else if trimmed.ends_with("+AND+") {
                        trimmed = trimmed[..trimmed.len() - 5].trim().to_string();
                    } else if trimmed.ends_with("+AND") {
                        trimmed = trimmed[..trimmed.len() - 4].trim().to_string();
                    }
                    trimmed
                } else {
                    geturl.clone()
                }
            };
            debug!("Generated OIM Request URL: {}", newurl);
            //DATA get from rest api
            let data = httprequests::get_data(
                &client,
                &newurl,
                &conf.username,
                &conf.password,
                conf.entries,
            )
            .await
            .map_err(|e| {
                tonic::Status::new(
                    tonic::Code::NotFound,
                    format!("Getting data failed: {:?}", e),
                )
            })?;

            //HEADER data extract
            let mut hm: HashMap<String, Series> =
                datapolars::getheaders(&client, &geturl, &conf.username, &conf.password)
                    .await
                    .map_err(|e| {
                        tonic::Status::new(
                            tonic::Code::ResourceExhausted,
                            format!("Header extract failed: {:?}", e),
                        )
                    })?;

            //FILL series with data
            let data = datapolars::fillseries(data, &mut hm).clone();

            //DATAFRAME create
            let mut df_append = DataFrame::default();
            for (_i, v) in data {
                //convert series to dataframe
                let df = v.into_frame();
                //concat dataframe
                df_append = concat_df_horizontal(&[df_append, df]).map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::NotFound,
                        format!("Creating dataframe failed: {:?}", e),
                    )
                })?;
            }

            //HEADER for select
            let df_header = vec![
                "Process Instance.Task Information.Creation Date",
                "Objects.Name",
                "Process Instance.Task Details.Key",
                "Process Definition.Tasks.Task Name",
                "Process Instance.Task Information.Target User",
            ];

            //BUILD dataframe
            let df = pl_vstr_to_selects(df_append, df_header).map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Aborted,
                    format!("Building dataframe failed: {:?}", e),
                )
            })?;
            let mut out = datapolars::get_data(df, &conf.filter1, &conf.filter2).map_err(|e| {
                tonic::Status::new(
                    tonic::Code::InvalidArgument,
                    format!("Filtering failed: {:?}", e),
                )
            })?;

            if limit > 0 {
                let remaining = (limit - tasks_written) as usize;
                if out.height() > remaining {
                    out = out.slice(0, remaining);
                }
            }
            let num_rows = out.height();

            if is_direct_mode {
                for idx in 0..num_rows {
                    let row = out.get_row(idx).unwrap().0;
                    response_tasks.push(proto::TaskDetails {
                        key: row[2].to_string(),
                        object_name: row[1].to_string(),
                        task_name: row[3].to_string(),
                        target_user: row[4].to_string(),
                        creation_date: row[0].to_string(),
                    });
                }
            } else if !is_db_mode {
                info!("DB is None/CSV Mode, writing to CSV");

                //CSV write
                let mut file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;

                //CSV write header
                file_header(file, path, out)?;

                //CSV read
                let contents =
                    fs::read_to_string(path).expect("Should have been able to read the file");

                //CSV read
                let splitted: Vec<&str> = contents.split('\n').collect();
                debug!("Ids: {:?}", splitted);
            } else {
                info!("DB access");
                for idx in 0..num_rows {
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
                }
            }

            tasks_written += num_rows as i32;
        }

        let now = Utc::now();
        let timestamp = Timestamp {
            seconds: now.timestamp(),
            nanos: now.timestamp_subsec_nanos() as i32,
        };

        let count = if is_direct_mode {
            response_tasks.len() as i32
        } else if is_db_mode {
            let db = self.db.as_ref().unwrap().lock().await;
            if let Some(surreal_client) = &db.db {
                let mut response = surreal_client
                    .query("SELECT * FROM task")
                    .await
                    .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{:?}", e)))?;
                let tasks: Vec<Task> = response
                    .take(0)
                    .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{:?}", e)))?;
                tasks.len() as i32
            } else {
                0
            }
        } else {
            if !Path::new(&path).exists() || std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) == 0 {
                0
            } else {
                let taskstosubmit = CsvReader::from_path(&path)
                    .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?
                    .finish()
                    .unwrap()["Process Instance.Task Details.Key"]
                    .as_list()
                    .clone();
                taskstosubmit.len() as i32
            }
        };

        Ok(tonic::Response::new(proto::ListResponse {
            result: count,
            time: Some(timestamp),
            message: String::from("List generated successfully"),
            tasks: response_tasks,
        }))
    }

    async fn prov_action(
        &self,
        request: tonic::Request<proto::ProvAcionRequest>,
    ) -> Result<tonic::Response<proto::Dictionary>, tonic::Status> {
        //MATCH action with enum Protobuf to ENUM
        let action = action_mapper(request)?;

        //CONFIG data from state
        let conf = self.get_config().await?;
        info!("Action: {:?}", action);
        let db_mod = conf.db;
        let conf = &conf.grpc;
        let path = conf.filelist.clone();

        //CLIENT SETUP
        let client = rest_client(conf.timeout)
            .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{:?}", e)))?;

        if !db_mod {
            //LOAD data from CSV
            let taskstosubmit = data_load(&path)?;

            //LOOP setup
            //LIST of retried tasks
            let mut tasks_retried: HashMap<String, proto::Task> = HashMap::new();
            //LIST interate
            for i in &taskstosubmit {
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

                // Perform the HTTP retry request using helper function
                perform_task_retry(
                    &client,
                    conf,
                    &id.to_string(),
                    action.clone(),
                    &mut tasks_retried,
                )
                .await;

                //LIST POP First
                let df = CsvReader::from_path(&path).unwrap().finish().unwrap();
                let length = df["Process Instance.Task Details.Key"].len() as u32;
                let mut df_a = df
                    .clone()
                    .lazy()
                    .slice(1, length - 1)
                    .collect()
                    .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;
                let mut file = std::fs::File::create(&path)
                    .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;
                CsvWriter::new(&mut file).finish(&mut df_a).unwrap();

                thread::sleep(Duration::from_millis(conf.sleep));
            }
            let response = proto::Dictionary {
                pairs: tasks_retried,
            };
            Ok(tonic::Response::new(response))
        } else {
            //SELECT FROM DB AND PERFORM RETRIES
            let mut tasks_retried: HashMap<String, proto::Task> = HashMap::new();
            loop {
                if conf.checkmode {
                    break;
                }
                let db = self.db.as_ref().unwrap().lock().await;
                let ii = match db.db_get_first_row("task").await {
                    Ok(task) => task,
                    Err(_) => break, // Break when no more tasks
                };
                //Process Instance.Task Details.Key
                let task_id = ii.process_instance_task_details_key.clone();
                let entry_id = ii.id.expect("Task should have an ID");

                // Perform the HTTP retry request using helper function
                perform_task_retry(&client, conf, &task_id, action.clone(), &mut tasks_retried)
                    .await;

                // Delete entry from SurrealDB
                db.db_delete_by_id(entry_id)
                    .await
                    .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;

                thread::sleep(Duration::from_millis(conf.sleep));
            }
            Ok(tonic::Response::new(proto::Dictionary {
                pairs: tasks_retried,
            }))
        }
    }
}

mod tests {

    /* #[test]
    fn urlsbuilder_test() {} */
}
