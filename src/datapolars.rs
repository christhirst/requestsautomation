use crate::error::CliError;
use crate::{http::httprequests, model::Task};

use polars::{
    datatypes::{DataType, TimeUnit},
    error::PolarsError,
    frame::DataFrame,
    lazy::{
        dsl::{col, lit, StrptimeOptions},
        frame::IntoLazy,
    },
    prelude::{NamedFrom, SortMultipleOptions},
    series::Series,
};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use tracing::info;

pub fn get_data(df: DataFrame, filter1: &str, filter2: &str) -> Result<DataFrame, PolarsError> {
    let out = df
        .clone()
        .lazy()
        .filter(
            polars::lazy::dsl::col("Objects.Name")
                .str()
                .contains(lit(filter1), false),
        )
        .filter(
            col("Process Definition.Tasks.Task Name")
                .str()
                .contains(lit(filter2), false),
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
            ["Process Instance.Task Information.Creation Date"],
            SortMultipleOptions {
                descending: vec![false],
                nulls_last: true,
                ..Default::default()
            },
        )
        .collect()?;
    Ok(out)
}

pub fn pl_vstr_to_selects(df: DataFrame, filter: Vec<&str>) -> Result<DataFrame, PolarsError> {
    let mut eer: Vec<polars::lazy::dsl::Expr> = vec![];
    for f in filter {
        eer.push(col(f))
    }

    let df = df.clone().lazy().select(eer).collect()?;
    Ok(df)
}

pub async fn getheaders(
    client: &Client,
    url: &str,
    username: &str,
    password: &str,
) -> Result<HashMap<String, Series>, CliError> {
    info!("headersurl: {:?}", url);
    let data = httprequests::get_data(client, url, username, password, 1).await?;
    let mut headers = vec!["".to_owned()];

    if let Some(ii) = data.clone().into_iter().next() {
        for iii in ii.fields {
            headers.push(iii.name.to_owned())
        }
    };
    let mut hm: HashMap<String, Series> = HashMap::from([]);
    info!("headers: {:?}", headers);

    for header in headers {
        let v1: Vec<String> = vec![];
        let series = Series::new(header.as_str(), v1);
        hm.entry(header).or_insert(series);
    }

    Ok(hm)
}

pub fn fillseries(
    data: Vec<Task>,
    hm: &mut HashMap<String, Series>,
) -> &mut HashMap<String, Series> {
    for i in data {
        for iii in &i.fields {
            let a = iii.name.clone();
            match iii.value.clone() {
                Value::Number(v) => {
                    let s = Series::new(&a, vec![v.to_string()]);
                    let oo = hm.get_mut(&a);
                    if let Some(x) = oo {
                        let _ = x.append(&s);
                    }
                }
                Value::String(v) => {
                    let s = Series::new(&a, vec![v.as_str()]);
                    let oo = hm.get_mut(&a);
                    if let Some(x) = oo {
                        let _ = x.append(&s);
                    }
                }
                _ => panic!("Type is wrong in value:Value matching"),
            };
        }
    }
    hm
}
