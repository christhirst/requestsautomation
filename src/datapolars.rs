use crate::error::CliError;
use crate::model::Task;

use polars::{
    datatypes::{DataType, TimeUnit},
    error::PolarsError,
    frame::DataFrame,
    lazy::{
        dsl::{StrptimeOptions, col, lit},
        frame::IntoLazy,
    },
    prelude::{NamedFrom, SortMultipleOptions},
    series::Series,
};
use serde_json::Value;

pub const COL_CREATION_DATE: &str = "Process Instance.Task Information.Creation Date";
pub const COL_OBJECT_NAME: &str = "Objects.Name";
pub const COL_TASK_KEY: &str = "Process Instance.Task Details.Key";
pub const COL_TASK_NAME: &str = "Process Definition.Tasks.Task Name";
pub const COL_TARGET_USER: &str = "Process Instance.Task Information.Target User";


fn json_value_to_string(val: &Value) -> String {
    match val {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Converts a slice of OIM `Task` objects directly into a Polars `DataFrame`
/// containing the 5 standard provisioning task columns without extra HTTP calls.
pub fn tasks_to_dataframe(tasks: &[Task]) -> Result<DataFrame, CliError> {
    let cap = tasks.len();
    let mut creation_dates = Vec::with_capacity(cap);
    let mut object_names = Vec::with_capacity(cap);
    let mut task_keys = Vec::with_capacity(cap);
    let mut task_names = Vec::with_capacity(cap);
    let mut target_users = Vec::with_capacity(cap);

    for task in tasks {
        let mut c_date = String::new();
        let mut obj_name = String::new();
        let mut t_key = String::new();
        let mut t_name = String::new();
        let mut t_user = String::new();

        for field in &task.fields {
            match field.name.as_str() {
                COL_CREATION_DATE => c_date = json_value_to_string(&field.value),
                COL_OBJECT_NAME => obj_name = json_value_to_string(&field.value),
                COL_TASK_KEY => t_key = json_value_to_string(&field.value),
                COL_TASK_NAME => t_name = json_value_to_string(&field.value),
                COL_TARGET_USER => t_user = json_value_to_string(&field.value),
                _ => {}
            }
        }

        creation_dates.push(c_date);
        object_names.push(obj_name);
        task_keys.push(t_key);
        task_names.push(t_name);
        target_users.push(t_user);
    }

    let df = DataFrame::new(vec![
        Series::new(COL_CREATION_DATE, creation_dates),
        Series::new(COL_OBJECT_NAME, object_names),
        Series::new(COL_TASK_KEY, task_keys),
        Series::new(COL_TASK_NAME, task_names),
        Series::new(COL_TARGET_USER, target_users),
    ])?;

    Ok(df)
}

/// Filters a provisioning task `DataFrame` by object name and task name,
/// casts the key and creation date columns, and sorts chronologically.
pub fn filter_and_sort_tasks(
    df: DataFrame,
    filter_object: &str,
    filter_task: &str,
    descending: bool,
) -> Result<DataFrame, PolarsError> {
    if df.height() == 0 {
        return Ok(df);
    }

    let mut lf = df.lazy();

    if !filter_object.is_empty() {
        lf = lf.filter(col(COL_OBJECT_NAME).str().contains(lit(filter_object), false));
    }

    if !filter_task.is_empty() {
        lf = lf.filter(col(COL_TASK_NAME).str().contains(lit(filter_task), false));
    }

    let sort_options = SortMultipleOptions::default().with_order_descending(descending);

    let out = lf
        .with_columns([col(COL_CREATION_DATE)
            .str()
            .replace(lit(r"Z$"), lit(""), false)
            .str()
            .strptime(
                DataType::Datetime(TimeUnit::Milliseconds, None),
                StrptimeOptions {
                    format: None,
                    strict: false,
                    exact: false,
                    cache: false,
                },
                lit("raise"),
            )])
        .with_columns([col(COL_TASK_KEY).cast(DataType::Int64)])
        .sort([COL_CREATION_DATE], sort_options)
        .collect()?;

    Ok(out)
}

#[allow(dead_code)]
pub fn get_data(df: DataFrame, filter1: &str, filter2: &str) -> Result<DataFrame, PolarsError> {
    filter_and_sort_tasks(df, filter1, filter2, false)
}

#[allow(dead_code)]
pub fn pl_vstr_to_selects(df: DataFrame, filter: Vec<&str>) -> Result<DataFrame, PolarsError> {
    let exprs: Vec<polars::lazy::dsl::Expr> = filter.into_iter().map(col).collect();
    df.lazy().select(exprs).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Field;
    use serde_json::json;

    fn sample_task(key: i64, obj: &str, task_name: &str, user: &str, date: &str) -> Task {
        Task {
            links: vec![],
            fields: vec![
                Field {
                    name: COL_CREATION_DATE.to_string(),
                    value: json!(date),
                },
                Field {
                    name: COL_OBJECT_NAME.to_string(),
                    value: json!(obj),
                },
                Field {
                    name: COL_TASK_KEY.to_string(),
                    value: json!(key),
                },
                Field {
                    name: COL_TASK_NAME.to_string(),
                    value: json!(task_name),
                },
                Field {
                    name: COL_TARGET_USER.to_string(),
                    value: json!(user),
                },
            ],
        }
    }

    #[test]
    fn tasks_to_dataframe_and_sort_chronologically() {
        let tasks = vec![
            sample_task(
                622485,
                "AAccount",
                "Update Number",
                "qquserid",
                "2024-01-04T14:51:04Z",
            ),
            sample_task(
                622483,
                "AAccount",
                "Update Number",
                "ttuserid",
                "2024-01-02T14:51:04Z",
            ),
            sample_task(
                622484,
                "OtherApp",
                "Create User",
                "aauserid",
                "2024-01-03T14:51:04Z",
            ),
        ];

        let df = tasks_to_dataframe(&tasks).unwrap();
        assert_eq!(df.height(), 3);

        // Filter for AAccount & Update, sorted ascending by Creation Date
        let sorted = filter_and_sort_tasks(df.clone(), "AAccount", "Update", false).unwrap();
        assert_eq!(sorted.height(), 2);

        let keys: Vec<i64> = sorted
            .column(COL_TASK_KEY)
            .unwrap()
            .i64()
            .unwrap()
            .into_no_null_iter()
            .collect();
        assert_eq!(keys, vec![622483, 622485]);

        // Sort descending
        let sorted_desc = filter_and_sort_tasks(df, "AAccount", "Update", true).unwrap();
        let keys_desc: Vec<i64> = sorted_desc
            .column(COL_TASK_KEY)
            .unwrap()
            .i64()
            .unwrap()
            .into_no_null_iter()
            .collect();
        assert_eq!(keys_desc, vec![622485, 622483]);
    }

    #[test]
    fn empty_tasks_dataframe_handled_gracefully() {
        let df = tasks_to_dataframe(&[]).unwrap();
        assert_eq!(df.height(), 0);
        let sorted = filter_and_sort_tasks(df, "AAccount", "Update", false).unwrap();
        assert_eq!(sorted.height(), 0);
    }
}
