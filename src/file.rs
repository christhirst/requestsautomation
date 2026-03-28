use std::{fs::File, path::Path};

use polars::{
    frame::DataFrame,
    io::SerWriter,
    prelude::{CsvWriter, CsvWriterOptions},
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
