use std::{
    fs::{File, OpenOptions},
    path::Path,
};

use polars::{
    frame::DataFrame,
    io::{SerReader, SerWriter},
    lazy::frame::IntoLazy,
    prelude::{CsvReader, CsvWriter},
};

use crate::datapolars::COL_TASK_KEY;

pub fn file_header(
    mut file: File,
    path: &str,
    mut out: DataFrame,
) -> Result<DataFrame, tonic::Status> {
    if out.height() == 0 {
        return Ok(out);
    }
    let include_header = std::fs::metadata(path).map(|m| m.len() == 0).unwrap_or(true);
    CsvWriter::new(&mut file)
        .include_header(include_header)
        .finish(&mut out)
        .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{e:?}")))?;
    Ok(out)
}

/// Appends a `DataFrame` to the CSV file at `path`, writing the header only if the file is new or empty.
pub fn append_tasks_to_csv(path: &str, out: DataFrame) -> Result<(), tonic::Status> {
    if out.height() == 0 {
        return Ok(());
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{e:?}")))?;
    file_header(file, path, out)?;
    Ok(())
}

/// Reads all task keys (`Process Instance.Task Details.Key`) from the CSV file at `path`.
pub fn load_task_keys(path: &str) -> Result<Vec<String>, tonic::Status> {
    if !Path::new(path).exists() || std::fs::metadata(path).map(|m| m.len()).unwrap_or(0) == 0 {
        return Ok(vec![]);
    }

    let df = CsvReader::from_path(path)
        .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{e:?}")))?
        .finish()
        .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{e:?}")))?;

    if df.height() == 0 {
        return Ok(vec![]);
    }

    let col = df
        .column(COL_TASK_KEY)
        .map_err(|e| tonic::Status::new(tonic::Code::InvalidArgument, format!("{e:?}")))?;

    let keys: Vec<String> = col
        .iter()
        .map(|av| av.to_string().trim_matches('"').to_string())
        .filter(|s| !s.is_empty() && s != "null")
        .collect();

    Ok(keys)
}

/// Removes the first row from the CSV file at `path` after a task has been processed.
pub fn pop_first_csv_row(path: &str) -> Result<(), tonic::Status> {
    if !Path::new(path).exists() || std::fs::metadata(path).map(|m| m.len()).unwrap_or(0) == 0 {
        return Ok(());
    }

    let df = CsvReader::from_path(path)
        .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{e:?}")))?
        .finish()
        .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{e:?}")))?;

    let length = df.height();
    if length <= 1 {
        let mut empty_df = df.clear();
        let mut file = File::create(path)
            .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{e:?}")))?;
        CsvWriter::new(&mut file)
            .include_header(true)
            .finish(&mut empty_df)
            .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{e:?}")))?;
        return Ok(());
    }

    let mut remaining_df = df
        .lazy()
        .slice(1, (length - 1) as u32)
        .collect()
        .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{e:?}")))?;

    let mut file = File::create(path)
        .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{e:?}")))?;
    CsvWriter::new(&mut file)
        .include_header(true)
        .finish(&mut remaining_df)
        .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{e:?}")))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::datapolars::{
        COL_CREATION_DATE, COL_OBJECT_NAME, COL_TARGET_USER, COL_TASK_KEY, COL_TASK_NAME,
    };
    use assert_fs::NamedTempFile;
    use polars::prelude::{NamedFrom, Series};

    #[test]
    fn csv_append_load_and_pop_lifecycle() {
        let temp = NamedTempFile::new("tasks_test.csv").unwrap();
        let path = temp.path().to_str().unwrap();

        let df = DataFrame::new(vec![
            Series::new(
                COL_CREATION_DATE,
                vec!["2024-01-02T14:51:04.000", "2024-01-03T14:51:04.000"],
            ),
            Series::new(COL_OBJECT_NAME, vec!["AAccount", "AAccount"]),
            Series::new(COL_TASK_KEY, vec![622483_i64, 622484_i64]),
            Series::new(COL_TASK_NAME, vec!["Update Number", "Update Number"]),
            Series::new(COL_TARGET_USER, vec!["ttuserid", "aauserid"]),
        ])
        .unwrap();

        append_tasks_to_csv(path, df).unwrap();

        let keys = load_task_keys(path).unwrap();
        assert_eq!(keys, vec!["622483".to_string(), "622484".to_string()]);

        pop_first_csv_row(path).unwrap();
        let keys_after_first_pop = load_task_keys(path).unwrap();
        assert_eq!(keys_after_first_pop, vec!["622484".to_string()]);

        pop_first_csv_row(path).unwrap();
        let keys_after_second_pop = load_task_keys(path).unwrap();
        assert!(keys_after_second_pop.is_empty());
    }
}
