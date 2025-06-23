use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb::RecordId;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Task {
    pub process_instance_task_information_creation_date: DateTime<Utc>,
    pub objects_name: String,
    pub process_instance_task_details_key: String,
    pub process_definition_tasks_task_name: String,
    pub process_instance_task_information_target_user: String,
}

#[derive(Debug, Deserialize)]
pub struct Record {
    pub id: RecordId,
}
