use polars::{
    chunked_array::ops::SortOptions,
    datatypes::{DataType, TimeUnit},
    error::PolarsError,
    frame::DataFrame,
    lazy::{
        dsl::{col, lit, StrptimeOptions},
        frame::IntoLazy,
    },
};

pub fn get_data(df: DataFrame) -> Result<DataFrame, PolarsError> {
    let mut out = df
        .clone()
        .lazy()
        .filter(
            polars::lazy::dsl::col("APP_INSTANCE_NAME")
                .str()
                .contains(lit("CAccount"), false),
        )
        .filter(
            col("Process Definition.Tasks.Task Name")
                .str()
                .contains(lit("Update"), false),
        )
        .with_columns([col("Process Instance.Task Information.Creation Date")
            .str()
            .strptime(
                DataType::Datetime(TimeUnit::Milliseconds, None),
                StrptimeOptions {
                    format: Some("%Y-%m-%dT%H:%M:%SZ".to_owned()),
                    strict: false,
                    exact: false,
                    cache: false,
                },
                lit("raise"),
            )])
        .with_columns([col("Process Instance.Task Details.Key").cast(DataType::Int64)])
        .sort(
            "Process Instance.Task Information.Creation Date",
            SortOptions {
                descending: false,
                nulls_last: true,
                ..Default::default()
            },
        )
        .collect()?;
    Ok(out)
}
