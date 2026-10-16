use std::{collections::HashMap, fs, path::Path, sync::Arc, time::Duration};

use chrono::{DateTime, NaiveDateTime, Utc};
use polars::datatypes::{AnyValue, TimeUnit};
use prost_types::Timestamp;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::Client;
use tokio::sync::{Mutex, RwLock};
use tracing::{debug, info, warn};

use crate::{
    Settings,
    config::{AppConfig, Database},
    dataendpoint::action_mapper,
    datapolars,
    db::types::Task,
    error::CliError,
    file::{append_tasks_to_csv, load_task_keys, pop_first_csv_row},
    http::{
        client::rest_client,
        httprequests::{self, build_query_url, resolve_url_filters},
    },
    types::ProvActionPayload,
};

use proto::user_server::User;

pub mod proto {
    tonic::include_proto!("requestsautomation");

    pub(crate) const FILE_DESCRIPTOR_SET: &[u8] =
        tonic::include_file_descriptor_set!("user_descriptor");
}

type State = Arc<RwLock<Option<Settings>>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageMode {
    Direct,
    Database,
    File,
}

impl StorageMode {
    pub fn from_request(mode: &str, default_db: bool) -> Self {
        match mode.trim().to_lowercase().as_str() {
            "direct" | "now" | "return" | "grpc" => Self::Direct,
            "db" | "database" | "surreal" | "surrealdb" => Self::Database,
            "csv" | "file" => Self::File,
            _ => {
                if default_db {
                    Self::Database
                } else {
                    Self::File
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct DBService {
    pub conf: Database,
    pub db: Option<Surreal<Client>>,
    pub jwt: Option<String>,
}

#[derive(Debug)]
pub struct UserService {
    pub state: State,
    pub db: Option<Arc<Mutex<DBService>>>,
}

impl UserService {
    async fn get_config(&self) -> Result<Settings, CliError> {
        debug!("Reading configuration from state");
        let guard = self.state.read().await;
        let settings = guard.as_ref().ok_or(CliError::EntityNotFound {
            entity: "Settings",
            id: 0,
        })?;
        Ok(settings.clone())
    }

    pub async fn new(conf: Settings) -> Result<Self, CliError> {
        let mut db = None;
        if conf.db {
            info!("Database persistence enabled");
            let dbs = DBService::new(conf.database.clone()).await?;
            db = Some(Arc::new(Mutex::new(dbs)));
        } else {
            info!("Database persistence disabled (defaulting to CSV/direct mode)");
        }

        let state = Arc::new(RwLock::new(Some(conf)));
        Ok(Self { state, db })
    }
}

fn anyvalue_to_clean_string(val: &AnyValue) -> String {
    match val {
        AnyValue::String(s) => (*s).to_string(),
        AnyValue::StringOwned(s) => s.to_string(),
        AnyValue::Datetime(ts, TimeUnit::Milliseconds, _) => {
            let secs = ts / 1000;
            let nsecs = ((ts % 1000).unsigned_abs() as u32) * 1_000_000;
            DateTime::from_timestamp(secs, nsecs)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| val.to_string())
        }
        AnyValue::Null => String::new(),
        other => other.to_string().trim_matches('"').to_string(),
    }
}

fn anyvalue_to_utc_datetime(val: &AnyValue) -> DateTime<Utc> {
    if let AnyValue::Datetime(ts, TimeUnit::Milliseconds, _) = val {
        let secs = ts / 1000;
        let nsecs = ((ts % 1000).unsigned_abs() as u32) * 1_000_000;
        if let Some(dt) = DateTime::from_timestamp(secs, nsecs) {
            return dt;
        }
    }

    let raw = anyvalue_to_clean_string(val);
    if let Ok(dt) = DateTime::parse_from_rfc3339(&raw) {
        return dt.with_timezone(&Utc);
    }
    let with_z = format!("{raw}Z");
    if let Ok(dt) = with_z.parse::<DateTime<Utc>>() {
        return dt;
    }
    if let Ok(naive) = NaiveDateTime::parse_from_str(&raw, "%Y-%m-%d %H:%M:%S%.f") {
        return naive.and_utc();
    }
    Utc::now()
}

async fn perform_task_action(
    client: &reqwest::Client,
    conf: &AppConfig,
    id: &str,
    action: &ProvActionPayload,
    tasks_processed: &mut HashMap<String, proto::Task>,
) {
    let puturl = format!("{}{}/{}", conf.baseurl, conf.urlput, id);
    debug!("Id: {id} PutUrl: {puturl} Action: {:?}", action);

    let mut status: u16 = 0;
    let mut attempt: i32 = 0;

    while attempt < 3 && status != 200 {
        attempt += 1;
        info!("Executing action '{}' on task {id} (attempt {attempt})", action.action);

        match httprequests::retrycall(
            client,
            &puturl,
            action.clone(),
            &conf.username,
            &conf.password,
        )
        .await
        {
            Ok(response) => {
                status = response.status().as_u16();
                info!("Task {id} HTTP status: {status}");
                tasks_processed.insert(
                    id.to_string(),
                    proto::Task {
                        id: id.to_string(),
                        status: response.status().to_string(),
                    },
                );
                if status != 200 {
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
            Err(e) => {
                warn!("Action '{}' failed for task {id}: {e:?}", action.action);
                tasks_processed.insert(
                    id.to_string(),
                    proto::Task {
                        id: id.to_string(),
                        status: "400".to_string(),
                    },
                );
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}

#[tonic::async_trait]
impl User for UserService {
    async fn conf_reload(
        &self,
        _request: tonic::Request<proto::UserRequest>,
    ) -> Result<tonic::Response<proto::ConfigResponse>, tonic::Status> {
        let conf = Settings::new().map_err(|e| {
            tonic::Status::new(
                tonic::Code::Internal,
                format!("Failed to load configuration: {e:?}"),
            )
        })?;

        let mut db_status = "Database disabled".to_string();
        if conf.db {
            if let Some(db_service) = &self.db {
                let mut db = db_service.lock().await;
                db.db_reload().await.map_err(|e| {
                    tonic::Status::new(
                        tonic::Code::Internal,
                        format!("Configuration reloaded, but database reload failed: {e:?}"),
                    )
                })?;
                db_status = "Database connection reloaded successfully".to_string();
            } else {
                db_status =
                    "Database enabled in config, but database service not initialized".to_string();
            }
        }

        let mut confs = self.state.write().await;
        *confs = Some(conf);

        Ok(tonic::Response::new(proto::ConfigResponse {
            result: format!("Reload successful. Status: {db_status}"),
        }))
    }

    async fn check_con(
        &self,
        _request: tonic::Request<proto::UserRequest>,
    ) -> Result<tonic::Response<proto::ConfigResponse>, tonic::Status> {
        let settings = self.get_config().await?;
        let url = format!("{}{}", settings.grpc.baseurl, settings.grpc.urlput);
        let client = rest_client(settings.grpc.timeout)?;
        let response = client
            .get(&url)
            .basic_auth(&settings.grpc.username, Some(&settings.grpc.password))
            .send()
            .await
            .map_err(|e| {
                tonic::Status::new(
                    tonic::Code::Unavailable,
                    format!("Connection failed to URL {url}: {e:?}"),
                )
            })?;

        let status_code = response.status();
        let is_success = status_code.is_success();
        let body = response
            .text()
            .await
            .unwrap_or_else(|e| format!("Failed to read response body: {e:?}"));

        let status_text = if is_success { "SUCCESS" } else { "FAILED" };
        let body_preview = if body.len() > 500 {
            format!("{}...", &body[..500])
        } else {
            body
        };

        let result_message = format!(
            "Connection Status: {}\nURL Checked: {}\nHTTP Status Code: {}\nResponse Body Preview:\n{}",
            status_text,
            url,
            status_code.as_u16(),
            body_preview
        );

        Ok(tonic::Response::new(proto::ConfigResponse {
            result: result_message,
        }))
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
            let db_arc = self.db.as_ref().ok_or_else(|| {
                tonic::Status::new(
                    tonic::Code::FailedPrecondition,
                    "Database is not initialized",
                )
            })?;
            let db = db_arc.lock().await;
            let deleted = db.db_delete_all("task").await?;
            return Ok(tonic::Response::new(proto::UserResponse {
                result: deleted.len() as i64,
            }));
        }

        let file = &settings.grpc.filelist;
        if Path::new(file).exists() {
            fs::remove_file(file)
                .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{e:?}")))?;
            info!("CSV file '{file}' deleted");
        }

        Ok(tonic::Response::new(proto::UserResponse { result: 1 }))
    }

    async fn prov_tasks_list(
        &self,
        request: tonic::Request<proto::FilterRequest>,
    ) -> Result<tonic::Response<proto::ListResponse>, tonic::Status> {
        let req = request.into_inner();
        info!("ProvTasksList Request: {:?}", req);

        let settings = self.get_config().await?;
        let conf = settings.grpc;
        let path = conf.filelist.clone();

        let storage_mode = StorageMode::from_request(&req.mode, self.db.is_some());
        if storage_mode == StorageMode::Database && self.db.is_none() {
            return Err(tonic::Status::new(
                tonic::Code::FailedPrecondition,
                "Database mode requested, but database is not enabled in settings",
            ));
        }

        let limit = req.limit;
        let urlget = if req.urlget.is_empty() {
            conf.urlget.clone().unwrap_or_default()
        } else {
            req.urlget
        };

        let filter_object = if req.filter_object.is_empty() {
            conf.filter1.clone()
        } else {
            req.filter_object
        };
        let filter_task = if req.filter_task.is_empty() {
            conf.filter2.clone()
        } else {
            req.filter_task
        };

        let urllist = resolve_url_filters(req.urlfilter, conf.urlfilter.as_ref());
        let client = rest_client(conf.timeout)?;
        let geturl = format!("{}{}{}", conf.baseurl, conf.urlput, urlget);
        debug!("Expanded filter queries: {:?}", urllist);

        let mut tasks_written = 0;
        let mut response_tasks = Vec::new();

        for buildurl in urllist {
            if limit > 0 && tasks_written >= limit {
                break;
            }

            let newurl = build_query_url(&geturl, &buildurl);
            debug!("Generated OIM Request URL: {}", newurl);

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
                    format!("Getting data from OIM failed: {e:?}"),
                )
            })?;

            let df = datapolars::tasks_to_dataframe(&data)?;
            let mut out = datapolars::filter_and_sort_tasks(
                df,
                &filter_object,
                &filter_task,
                req.sort_descending,
            )
            .map_err(|e| {
                tonic::Status::new(
                    tonic::Code::InvalidArgument,
                    format!("Filtering/sorting tasks failed: {e:?}"),
                )
            })?;

            if limit > 0 {
                let remaining = (limit - tasks_written) as usize;
                if out.height() > remaining {
                    out = out.slice(0, remaining);
                }
            }
            let num_rows = out.height();

            match storage_mode {
                StorageMode::Direct => {
                    for idx in 0..num_rows {
                        if let Ok(row) = out.get_row(idx) {
                            let vals = &row.0;
                            response_tasks.push(proto::TaskDetails {
                                creation_date: anyvalue_to_clean_string(&vals[0]),
                                object_name: anyvalue_to_clean_string(&vals[1]),
                                key: anyvalue_to_clean_string(&vals[2]),
                                task_name: anyvalue_to_clean_string(&vals[3]),
                                target_user: anyvalue_to_clean_string(&vals[4]),
                            });
                        }
                    }
                }
                StorageMode::File => {
                    info!("Writing {num_rows} tasks to CSV file '{path}'");
                    append_tasks_to_csv(&path, out)?;
                }
                StorageMode::Database => {
                    info!("Writing {num_rows} tasks to SurrealDB");
                    let db_arc = self.db.as_ref().unwrap();
                    for idx in 0..num_rows {
                        if let Ok(row) = out.get_row(idx) {
                            let vals = &row.0;
                            let task_row = Task {
                                id: None,
                                process_instance_task_information_creation_date:
                                    anyvalue_to_utc_datetime(&vals[0]),
                                objects_name: anyvalue_to_clean_string(&vals[1]),
                                process_instance_task_details_key: anyvalue_to_clean_string(
                                    &vals[2],
                                ),
                                process_definition_tasks_task_name: anyvalue_to_clean_string(
                                    &vals[3],
                                ),
                                process_instance_task_information_target_user:
                                    anyvalue_to_clean_string(&vals[4]),
                            };
                            let mut db = db_arc.lock().await;
                            db.db_create_entry("task", task_row).await.map_err(|e| {
                                tonic::Status::new(
                                    tonic::Code::Internal,
                                    format!("Failed to create SurrealDB task entry: {e:?}"),
                                )
                            })?;
                        }
                    }
                }
            }

            tasks_written += num_rows as i32;
        }

        let now = Utc::now();
        let timestamp = Timestamp {
            seconds: now.timestamp(),
            nanos: now.timestamp_subsec_nanos() as i32,
        };

        let count = match storage_mode {
            StorageMode::Direct => response_tasks.len() as i32,
            StorageMode::Database => {
                let db = self.db.as_ref().unwrap().lock().await;
                db.db_count_tasks().await?
            }
            StorageMode::File => load_task_keys(&path)?.len() as i32,
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
        request: tonic::Request<proto::ProvActionRequest>,
    ) -> Result<tonic::Response<proto::Dictionary>, tonic::Status> {
        let req = request.into_inner();
        let action_payload = action_mapper(&req)?;

        let settings = self.get_config().await?;
        let conf = &settings.grpc;
        let path = conf.filelist.clone();
        info!(
            "ProvAction requested: action='{}', mode='{}', direct_ids={:?}",
            action_payload.action, req.mode, req.task_ids
        );

        let client = rest_client(conf.timeout)?;
        let mut tasks_processed: HashMap<String, proto::Task> = HashMap::new();

        // 1. If explicit task_ids were provided (or mode == "direct"), process them directly
        if !req.task_ids.is_empty()
            || StorageMode::from_request(&req.mode, settings.db) == StorageMode::Direct
        {
            for id in &req.task_ids {
                if conf.checkmode {
                    break;
                }
                perform_task_action(&client, conf, id, &action_payload, &mut tasks_processed).await;
                if conf.sleep > 0 {
                    tokio::time::sleep(Duration::from_millis(conf.sleep)).await;
                }
            }
            return Ok(tonic::Response::new(proto::Dictionary {
                pairs: tasks_processed,
            }));
        }

        let storage_mode = StorageMode::from_request(&req.mode, settings.db);
        match storage_mode {
            StorageMode::File => {
                let task_keys = load_task_keys(&path)?;
                for id in task_keys {
                    if conf.checkmode {
                        break;
                    }
                    perform_task_action(&client, conf, &id, &action_payload, &mut tasks_processed)
                        .await;
                    pop_first_csv_row(&path)?;
                    if conf.sleep > 0 {
                        tokio::time::sleep(Duration::from_millis(conf.sleep)).await;
                    }
                }
            }
            StorageMode::Database => {
                let db_arc = self.db.as_ref().ok_or_else(|| {
                    tonic::Status::new(
                        tonic::Code::FailedPrecondition,
                        "Database mode requested, but database is not enabled in settings",
                    )
                })?;

                loop {
                    if conf.checkmode {
                        break;
                    }

                    // Lock only to read the oldest task, then release before network I/O
                    let next_task = {
                        let db = db_arc.lock().await;
                        db.db_get_first_row("task").await.ok()
                    };

                    let Some(task) = next_task else {
                        break;
                    };

                    let task_id = task.process_instance_task_details_key;
                    let entry_id = task.id.expect("Task from SurrealDB must have a RecordId");

                    perform_task_action(
                        &client,
                        conf,
                        &task_id,
                        &action_payload,
                        &mut tasks_processed,
                    )
                    .await;

                    {
                        let db = db_arc.lock().await;
                        db.db_delete_by_id(entry_id).await.map_err(|e| {
                            tonic::Status::new(tonic::Code::Internal, format!("{e:?}"))
                        })?;
                    }

                    if conf.sleep > 0 {
                        tokio::time::sleep(Duration::from_millis(conf.sleep)).await;
                    }
                }
            }
            StorageMode::Direct => {}
        }

        Ok(tonic::Response::new(proto::Dictionary {
            pairs: tasks_processed,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_mode_resolution() {
        assert_eq!(StorageMode::from_request("direct", false), StorageMode::Direct);
        assert_eq!(StorageMode::from_request("grpc", true), StorageMode::Direct);
        assert_eq!(StorageMode::from_request("db", false), StorageMode::Database);
        assert_eq!(StorageMode::from_request("surrealdb", false), StorageMode::Database);
        assert_eq!(StorageMode::from_request("csv", true), StorageMode::File);
        assert_eq!(StorageMode::from_request("file", true), StorageMode::File);
        assert_eq!(StorageMode::from_request("", true), StorageMode::Database);
        assert_eq!(StorageMode::from_request("", false), StorageMode::File);
    }

    #[test]
    fn anyvalue_datetime_conversion() {
        // 2024-01-02T14:51:04Z = 1704207064000 ms
        let av = AnyValue::Datetime(1_704_207_064_000, TimeUnit::Milliseconds, &None);
        let dt = anyvalue_to_utc_datetime(&av);
        assert_eq!(dt.timestamp(), 1_704_207_064);
        assert_eq!(anyvalue_to_clean_string(&av), "2024-01-02T14:51:04+00:00");
    }
}
