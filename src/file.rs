use std::{fs::File, path::Path};

use polars::{
    frame::DataFrame,
    io::{SerReader, SerWriter},
    prelude::{ChunkedArray, CsvReader, CsvWriter, CsvWriterOptions, ListType},
};

pub fn file_header(
    mut file: File,
    path: &str,
    mut out: DataFrame,
) -> Result<DataFrame, tonic::Status> {
    let fileexists = !Path::new(path).exists();
    CsvWriter::new(&mut file)
        .include_header(fileexists)
        .finish(&mut out)
        .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?;
    Ok(out)
}

pub fn data_load(path: &str) -> Result<ChunkedArray<ListType>, tonic::Status> {
    let taskstosubmit = CsvReader::from_path(path)
        .map_err(|e| tonic::Status::new(tonic::Code::NotFound, format!("{:?}", e)))?
        .finish()
        .unwrap()["Process Instance.Task Details.Key"]
        .as_list();
    Ok(taskstosubmit)
}
