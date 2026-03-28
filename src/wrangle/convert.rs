use chrono::{DateTime, Utc};
use num_traits::NumCast;
use polars::{frame::DataFrame, prelude::DataType};

#[allow(dead_code)]
fn df_to_vec<T>(df: DataFrame) -> (Vec<Vec<DateTime<Utc>>>, Vec<Vec<String>>, Vec<Vec<T>>)
where
    T: NumCast,
{
    let series_list = df.iter().collect::<Vec<_>>();
    let mut list_dt = vec![];
    let mut _list_f64: Vec<Vec<f64>> = vec![];
    let mut list_str = vec![];
    let list_t = vec![];
    for i in series_list {
        //TODO Do it with generics and T WHERE T:
        match i.dtype() {
            DataType::Datetime(_time_unit, _time_zone) => {
                println!("Data type is Datetime");
                let vec_datetime: Vec<DateTime<Utc>> = i
                    .datetime()
                    .expect("Expected Datetime column")
                    .into_iter()
                    .filter_map(|opt_ts| {
                        opt_ts.and_then(|ts| {
                            let secs = ts / 1000;
                            let nsecs = ((ts % 1000) * 1_000_000) as u32;
                            DateTime::from_timestamp(secs, nsecs)
                        })
                    })
                    .collect();
                list_dt.push(vec_datetime);
            }
            DataType::Int32 => {
                println!("Data type is Int32");
                let _vec: Vec<T> = i
                    .iter()
                    .map(|av| av.extract::<T>().unwrap()) // or handle errors as needed
                    .collect();
            }
            DataType::Float64 => {
                println!("Data type is Float64");
                let _vec: Vec<f64> = i
                    .iter()
                    .map(|av| av.extract::<f64>().unwrap()) // or handle errors as needed
                    .collect();
                //list_f64 = vec![_vec];
            }

            DataType::String => {
                println!("Data type is String");
                let vec: Vec<String> = i
                    .iter()
                    .map(|av| av.to_string().trim_matches('"').to_string())
                    .collect();
                list_str.push(vec);
            }
            _ => println!("Other data type: {:?}", i.dtype()),
        }
    }

    (list_dt, list_str, list_t)
}

#[allow(unused_imports)]
mod tests {
    use super::*;
    use polars::{
        io::SerReader,
        prelude::{CsvReader, DataType, TimeUnit},
    };

    #[test]
    fn urlsbuilder_test() {
        let mut df = CsvReader::from_path("path.csv").unwrap().finish().unwrap();
        let df = df
            .with_column(
                df.column("Process Instance.Task Information.Creation Date")
                    .unwrap()
                    .cast(&DataType::Datetime(TimeUnit::Milliseconds, None))
                    .unwrap(),
            )
            .unwrap();
        let dd = df_to_vec::<i32>(df.clone());
        assert_eq!(dd.0.len(), 1);
        assert_eq!(dd.1.len(), 3);
    }

    #[test]
    fn date_convert() {
        //2024-01-02T14:51:04.000
        let mut df = CsvReader::from_path("path.csv").unwrap().finish().unwrap();
        let df = df
            .with_column(
                df.column("Process Instance.Task Information.Creation Date")
                    .unwrap()
                    .cast(&DataType::Datetime(TimeUnit::Milliseconds, None))
                    .unwrap(),
            )
            .unwrap();
        let dd = df_to_vec::<i32>(df.clone());
        assert_eq!(dd.0.len(), 1);
        assert_eq!(dd.1.len(), 3);
    }
}
