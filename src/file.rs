use std::{fs::File, path::Path};

use polars::{
    frame::DataFrame,
    io::{SerReader, SerWriter},
    prelude::{ChunkedArray, CsvReader, CsvWriter, ListType},
};

pub fn file_header(
    mut file: File,
    path: &str,
    mut out: DataFrame,
) -> Result<DataFrame, tonic::Status> {
    let file_meta = std::fs::metadata(path);
    let include_header = match file_meta {
        Ok(m) => m.len() == 0,
        Err(_) => true,
    };
    CsvWriter::new(&mut file)
        .include_header(include_header)
        .finish(&mut out)
        .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;
    Ok(out)
}

pub fn data_load(path: &str) -> Result<ChunkedArray<ListType>, tonic::Status> {
    if !Path::new(path).exists() || std::fs::metadata(path).map(|m| m.len()).unwrap_or(0) == 0 {
        let empty_series = polars::prelude::Series::new_empty(
            "Process Instance.Task Details.Key",
            &polars::prelude::DataType::List(Box::new(polars::prelude::DataType::Int64)),
        );
        let taskstosubmit = empty_series.as_list().clone();
        return Ok(taskstosubmit);
    }
    let taskstosubmit = CsvReader::from_path(path)
        .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?
        .finish()
        .unwrap()["Process Instance.Task Details.Key"]
        .as_list()
        .clone();
    Ok(taskstosubmit)
}
