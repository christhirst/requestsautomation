use surrealdb::IndexedResults as Response;
use surrealdb::Surreal;
use surrealdb::engine::remote::ws::{Client, Ws, Wss};
use surrealdb::opt::auth::Root;
use surrealdb_types::RecordId;

use crate::config::Database;
use crate::db::types::{Record, Task};
use crate::error::CliError;
use crate::grpcserver::DBService;

async fn connect_surreal(db_conf: &Database) -> surrealdb::Result<(Surreal<Client>, String)> {
    let url = db_conf.url.trim();
    let client = if let Some(stripped) = url.strip_prefix("ws://") {
        Surreal::new::<Ws>(stripped).await?
    } else if let Some(stripped) = url.strip_prefix("wss://") {
        Surreal::new::<Wss>(stripped).await?
    } else if url.starts_with("127.0.0.1") || url.starts_with("localhost") || url.starts_with("0.0.0.0") {
        Surreal::new::<Ws>(url).await?
    } else {
        Surreal::new::<Wss>(url).await?
    };

    client
        .use_ns(&db_conf.namespace)
        .use_db(&db_conf.database)
        .await?;

    let jwt = client
        .signin(Root {
            username: db_conf.user.clone(),
            password: db_conf.password.clone(),
        })
        .await?;

    Ok((client, jwt.access.into_insecure_token()))
}

impl DBService {
    #[allow(dead_code)]
    pub async fn schemafull_init(&self) -> surrealdb::Result<Response> {
        let db = self.db.as_ref().expect("Database client not initialized");
        let result = db
            .query(
                "
                DEFINE FIELD process_instance_task_information_creation_date ON TABLE task TYPE datetime;
                DEFINE FIELD objects_name ON TABLE task TYPE string;
                DEFINE FIELD process_instance_task_details_key ON TABLE task TYPE string;
                DEFINE FIELD process_definition_tasks_task_name ON TABLE task TYPE string;
                DEFINE FIELD process_instance_task_information_target_user ON TABLE task TYPE string;
                ",
            )
            .await?;
        Ok(result)
    }

    pub async fn db_create_entry(
        &mut self,
        _resource: &str,
        task: Task,
    ) -> surrealdb::Result<Option<Task>> {
        if self.db.is_none() {
            self.db_reload().await?;
        }
        let db = self.db.as_ref().expect("DB should be initialized");

        let mut response = db
            .query(
                "
                CREATE task SET
                    process_instance_task_information_creation_date = <datetime>$creation_date,
                    objects_name = $objects_name,
                    process_instance_task_details_key = $task_key,
                    process_definition_tasks_task_name = $task_name,
                    process_instance_task_information_target_user = $target_user
                ",
            )
            .bind((
                "creation_date",
                task.process_instance_task_information_creation_date
                    .to_rfc3339(),
            ))
            .bind(("objects_name", task.objects_name))
            .bind(("task_key", task.process_instance_task_details_key))
            .bind(("task_name", task.process_definition_tasks_task_name))
            .bind((
                "target_user",
                task.process_instance_task_information_target_user,
            ))
            .await?;

        let created: Option<Task> = response.take(0)?;
        Ok(created)
    }

    pub async fn db_delete_all(&self, table: &str) -> Result<Vec<Task>, CliError> {
        let db = self.db.as_ref().ok_or_else(|| CliError::EntityNotFound {
            entity: "Database",
            id: 0,
        })?;
        let deleted: Vec<Task> = db.delete(table).await?;
        Ok(deleted)
    }

    pub async fn db_delete_by_id(&self, record: RecordId) -> surrealdb::Result<()> {
        let db = self.db.as_ref().expect("DB should be initialized");
        db.query("DELETE $id").bind(("id", record)).await?;
        Ok(())
    }

    pub async fn db_count_tasks(&self) -> Result<i32, CliError> {
        let Some(db) = &self.db else {
            return Ok(0);
        };
        let mut response = db.query("SELECT * FROM task").await?;
        let tasks: Vec<Task> = response.take(0)?;
        Ok(tasks.len() as i32)
    }

    #[allow(dead_code)]
    pub async fn db_list(&self) -> Result<Vec<Task>, CliError> {
        let db = self.db.as_ref().ok_or_else(|| CliError::EntityNotFound {
            entity: "Database",
            id: 0,
        })?;
        let mut response = db.query("SELECT * FROM task").await?;
        let tasks: Vec<Task> = response.take(0)?;
        Ok(tasks)
    }

    pub async fn db_get_first_row(&self, _task: &str) -> Result<Task, CliError> {
        let db = self.db.as_ref().ok_or_else(|| CliError::EntityNotFound {
            entity: "Database",
            id: 0,
        })?;
        let mut result: Vec<Task> = db
            .query("SELECT * FROM task ORDER BY process_instance_task_information_creation_date ASC LIMIT 1;")
            .await?
            .take(0)?;

        let res = result.pop().ok_or(CliError::EntityNotFound {
            entity: "Task",
            id: 0,
        })?;
        Ok(res)
    }

    #[allow(dead_code)]
    pub async fn db_delete_row_first(&self) -> Result<(), surrealdb::Error> {
        let db = self.db.as_ref().expect("DB should be initialized");
        let mut result: Vec<Record> = db
            .query("SELECT * FROM task ORDER BY process_instance_task_information_creation_date ASC LIMIT 1;")
            .await?
            .take(0)?;

        if let Some(record) = result.pop() {
            db.query("DELETE $id").bind(("id", record.id)).await?;
        }
        Ok(())
    }

    pub async fn db_reload(&mut self) -> surrealdb::Result<()> {
        let (client, token) = connect_surreal(&self.conf).await?;
        self.db = Some(client);
        self.jwt = Some(token);
        tracing::info!("Database reloaded successfully");
        Ok(())
    }

    pub async fn new(db_conf: Database) -> Result<Self, surrealdb::Error> {
        let (client, token) = connect_surreal(&db_conf).await?;
        Ok(DBService {
            conf: db_conf,
            db: Some(client),
            jwt: Some(token),
        })
    }
}
