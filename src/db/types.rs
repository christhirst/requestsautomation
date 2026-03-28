use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use surrealdb_types::{RecordId, SurrealValue};

#[derive(Debug, Serialize, Deserialize, Clone, SurrealValue)]
pub struct Task {
    pub id: Option<RecordId>,
    pub process_instance_task_information_creation_date: DateTime<Utc>,
    pub objects_name: String,
    pub process_instance_task_details_key: String,
    pub process_definition_tasks_task_name: String,
    pub process_instance_task_information_target_user: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize, SurrealValue)]
pub struct Record {
    pub id: RecordId,
}
