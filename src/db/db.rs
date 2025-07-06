use surrealdb::engine::remote::ws::{Ws, Wss};
use surrealdb::opt::auth::Root;
use surrealdb::{RecordId, Response, Surreal};

use crate::config::Database;
use crate::db::types::{Record, Task};
use crate::error::CliError;
use crate::grpcserver::DBService;

impl DBService {
    #[allow(dead_code)]
    pub async fn schemafull_init(&self) -> surrealdb::Result<Response> {
        let result = self
            .db
            .clone()
            .unwrap()
            .query(
                "
                DEFINE FIELD process_instance_task_information_creation_date ON TABLE task TYPE datetime;
                DEFINE FIELD objects_name ON TABLE task TYPE string;
                DEFINE FIELD process_instance_task_details_key ON TABLE task TYPE string;
                DEFINE FIELD process_definition_tasks_task_name ON TABLE task TYPE string;
                DEFINE FIELD process_instance_task_information_target_user ON TABLE task TYPE string;
                ",
            )
            .await
            .unwrap();
        Ok(result)
    }

    pub async fn db_create_entry(
        &mut self,
        resource: &str,
        task: Task,
    ) -> surrealdb::Result<Option<Task>> {
        // Create a new person with a random id
        let db = match self.db.as_ref() {
            Some(db) => db,
            None => {
                self.db_reload().await?;
                self.db.as_ref().expect("DB should be reloaded")
            }
        };

        let bad_sql = format!(
            "
            CREATE task SET
            process_instance_task_information_creation_date = d'{}',
            objects_name = '{}',
            process_instance_task_details_key = '{}',
            process_definition_tasks_task_name = '{}',
            process_instance_task_information_target_user = '{}'
            ",
            task.process_instance_task_information_creation_date
                .to_rfc3339(),
            task.objects_name,
            task.process_instance_task_details_key,
            task.process_definition_tasks_task_name,
            task.process_instance_task_information_target_user
        );

        let created: Option<Task> = self
            .db
            .as_ref()
            .unwrap()
            .query(bad_sql)
            .await
            .unwrap()
            .take(0)
            .unwrap();

        Ok(created)
    }

    #[allow(dead_code)]
    pub async fn db_delete_all(&self, task: &str) -> Result<Vec<Task>, CliError> {
        let deleted_rest: Vec<Task> = self.db.as_ref().unwrap().delete(task).await?;
        Ok(deleted_rest)
    }
    pub async fn db_delete_by_id(&self, record: RecordId) -> surrealdb::Result<()> {
        let db = self.db.as_ref().unwrap();

        db.query("DELETE $id").bind(("id", record)).await?;

        Ok(())
    }
    #[allow(dead_code)]
    pub async fn db_list(&self) -> Result<(), CliError> {
        let result: Vec<Task> = self
            .db
            .clone()
            .unwrap()
            .query("SELECT * FROM task")
            .await
            .unwrap()
            .take(0)
            .unwrap();
        Ok(())
    }

    pub async fn db_get_first_row(&self, task: &str) -> Result<Task, CliError> {
        let db = self.db.as_ref().unwrap();
        let mut result: Vec<Task> = db
            .query("SELECT * FROM task ORDER BY process_instance_task_information_creation_date ASC LIMIT 1;")
            .await?
            .take(0)?;
        println!("Result: {:?}", result);
        let res = result.pop().ok_or_else(|| {
            CliError::EntityNotFound {
                entity: "Task",
                id: 0, // Placeholder ID, as we don't have an ID here
            }
        })?;
        Ok(res)
    }

    #[allow(dead_code)]
    pub async fn db_delete_row_first(&self) -> Result<(), surrealdb::Error> {
        //let deleted_one: Option<Task> = self.db.as_ref().unwrap().delete(("person", "one")).await?;
        let db = self.db.as_ref().unwrap();
        let mut result: Vec<Record> = db
            .query("SELECT * FROM task ORDER BY process_instance_task_information_creation_date ASC LIMIT 1;")
            .await?
            .take(0)?; // Get ID

        if let Some(id) = result.pop() {
            db.query("DELETE $id").bind(("id", id.id)).await?;
        }
        Ok(())
    }

    pub async fn db_reload(&mut self) -> surrealdb::Result<()> {
        let db_conf = self.conf.clone();
        //let db_conf = db_conf.as_ref().unwrap().database.clone();
        let client = Surreal::new::<Ws>(db_conf.url).await?;
        client
            .use_ns(&db_conf.namespace)
            .use_db(&db_conf.database)
            .await?;
        //let _ = client.authenticate(db_conf.token).await.is_ok();
        let jwt = client
            .signin(Root {
                username: &db_conf.user,
                password: &db_conf.password,
            })
            .await?;
        self.db = Some(client);
        self.jwt = Some(jwt.into_insecure_token());
        tracing::info!("Database reloaded successfully");
        Ok(())
    }
    pub async fn new(db_conf: Database) -> Result<Self, surrealdb::Error> {
        let conf = db_conf.clone();
        let client = Surreal::new::<Wss>(db_conf.url).await?;
        client
            .use_ns(&db_conf.namespace)
            .use_db(&db_conf.database)
            .await?;
        //let _ = client.authenticate(db_conf.token).await.is_ok();
        let jwt = client
            .signin(Root {
                username: &db_conf.user,
                password: &db_conf.password,
            })
            .await?;
        Ok(DBService {
            conf: conf,
            db: Some(client),
            jwt: Some(jwt.into_insecure_token()),
        })
    }
}

/* #[cfg(test)]
mod tests {
    use chrono::Utc;

    use crate::{config::Settings, db::types::Task};

    use super::*;
    #[tokio::test]
    async fn db_new() -> Result<(), Box<dyn std::error::Error>> {
        let settings = Settings::new().unwrap().database;
        let res = DBService::new(settings).await?;

        assert!(
            res.db.is_some(),
            "Database connection should be established"
        );
        Ok(())
    }
    #[tokio::test]
    async fn db_create() -> Result<(), Box<dyn std::error::Error>> {
        let settings = Settings::new().unwrap().database;
        let mut res = DBService::new(settings).await?;

        let oo: Task = Task {
            id: None,
            process_instance_task_information_creation_date: Utc::now(),
            objects_name: String::from("AAccount"),
            process_instance_task_details_key: String::from("333"),
            process_definition_tasks_task_name: String::from("Update Number"),
            process_instance_task_information_target_user: String::from("ttuserid"),
        };

        //db_con()?;
        let res = res.db_create_entry("task", oo).await?;
        //panic!("Test failed, this is a panic to test the error handling in the test framework");
        /* assert!(
            res.db.is_some(),
            "Database connection should be established"
        ); */
        Ok(())
    }
    #[tokio::test]
    async fn db_list() -> Result<(), Box<dyn std::error::Error>> {
        let settings = Settings::new().unwrap().database;
        let res = DBService::new(settings).await?;
        let res = res.db_list().await?;

        assert!(!res.is_empty(), "Database list should not be empty");
        Ok(())
    }

    #[tokio::test]
    async fn db_get_first_row() -> Result<(), Box<dyn std::error::Error>> {
        let settings = Settings::new().unwrap().database;
        let res = DBService::new(settings).await?;
        let res = res.db_get_first_row("task").await?;
        println!("{:?}", res);
        assert_eq!(
            res.process_definition_tasks_task_name,
            "Database delete should be successful"
        );
        Ok(())
    }

    #[tokio::test]
    async fn db_del() -> Result<(), Box<dyn std::error::Error>> {
        let settings = Settings::new().unwrap().database;
        let res = DBService::new(settings).await?;
        let res = res.db_delete_all("task").await?;

        //assert!(res.is_ok(), "Database delete should be successful");
        Ok(())
    }
    #[tokio::test]
    async fn db_del_row_first() -> Result<(), Box<dyn std::error::Error>> {
        let settings = Settings::new().unwrap().database;
        let res = DBService::new(settings).await?;
        let res = res.db_delete_row_first().await?;

        //assert!(res.is_ok(), "Database delete should be successful");
        Ok(())
    }

    #[tokio::test]
    async fn db_schema() -> Result<(), Box<dyn std::error::Error>> {
        let settings = Settings::new().unwrap().database;
        let res = DBService::new(settings).await?;
        let res = res.schemafull_init().await?;

        //assert!(res.is_ok(), "Database delete should be successful");
        Ok(())
    }
}
 */
