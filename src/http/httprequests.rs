use std::{collections::HashMap, time::Duration};

use reqwest::{
    Client, Response,
    header::{ACCEPT, CONTENT_TYPE},
};
use tracing::{debug, info};

use crate::{
    error::CliError,
    grpcserver::proto::FilterValues,
    model::{Root, Roots, Task},
    types::ProvActionPayload,
};

pub async fn get_data(
    client: &Client,
    url: &str,
    username: &str,
    password: &str,
    fetched: u32,
) -> Result<Vec<Task>, CliError> {
    let mut alltasks: Vec<Task> = vec![];
    let mut more = true;
    let mut count = 0;
    let mut current_url = url.to_string();

    while more && count < fetched {
        let data = fetchdata(client, &current_url, username, password).await?;
        debug!("Entries in Backend: {:?}", data);
        match data {
            Roots::Root(d) => {
                more = extract_tasks_and_next_link(d, &mut alltasks, &mut current_url);
            }
        }
        count += 1;
    }

    Ok(alltasks)
}

fn extract_tasks_and_next_link(data: Root, alltasks: &mut Vec<Task>, url: &mut String) -> bool {
    if let Some(mut tasks) = data.tasks {
        alltasks.append(&mut tasks);
    }
    let more = data.has_more;
    for l in data.links {
        if l.rel == "next" {
            *url = l.href;
        }
    }
    info!("{}/{}", data.count, data.total_result);
    more
}

async fn fetchdata(
    client: &Client,
    url: &str,
    username: &str,
    password: &str,
) -> Result<Roots, CliError> {
    debug!("Fetching data from: {}", url);
    let response = client
        .get(url)
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json")
        .basic_auth(username, Some(password))
        .send()
        .await?;
    let t: Root = response.json().await?;
    debug!("Total result: {}", t.total_result);
    Ok(Roots::Root(t))
}

pub async fn retrycall(
    client: &Client,
    url: &str,
    body: ProvActionPayload,
    username: &str,
    password: &str,
) -> Result<Response, CliError> {
    debug!("PUT request to OIM API: {} with body: {:?}", url, body);
    let response = client
        .put(url)
        .json(&body)
        .header("X-Requested-By", "rust")
        .basic_auth(username, Some(password))
        .timeout(Duration::from_secs(5))
        .send()
        .await?;
    Ok(response)
}

/// Expands ordered `(attribute, values)` pairs into their Cartesian `AND` filter combinations.
///
/// For example:
/// `[[ "aa::bb::cc", ["AA","BB","CC"] ], [ "ggggg", ["aa","bb","cc"] ]]`
/// expands into:
/// - `"aa::bb::cc+eq+AA AND ggggg+eq+aa"`
/// - `"aa::bb::cc+eq+AA AND ggggg+eq+bb"`
/// - ...
pub fn urlsbuilder(urlfilter: &[(String, Vec<String>)]) -> Vec<String> {
    let mut uri_groups: Vec<Vec<String>> = Vec::new();
    for (attr, values) in urlfilter {
        let group: Vec<String> = values
            .iter()
            .filter(|v| !v.is_empty())
            .map(|val| format!("{attr}+eq+{val}"))
            .collect();
        if !group.is_empty() {
            uri_groups.push(group);
        }
    }

    if uri_groups.is_empty() {
        return vec![String::new()];
    }

    let mut combined = uri_groups[0].clone();
    for group in uri_groups.iter().skip(1) {
        let mut next_combined = Vec::with_capacity(combined.len() * group.len());
        for item1 in &combined {
            for item2 in group {
                next_combined.push(format!("{item1} AND {item2}"));
            }
        }
        combined = next_combined;
    }

    combined
}

/// Builds combined OIM SCIM filter expressions from a gRPC `map<string, FilterValues>`.
/// Keys are sorted lexicographically for deterministic query generation.
pub fn filterbuilder(urlfilter: HashMap<String, FilterValues>) -> Vec<String> {
    let mut keys: Vec<String> = urlfilter.keys().cloned().collect();
    keys.sort();

    let ordered_pairs: Vec<(String, Vec<String>)> = keys
        .into_iter()
        .filter_map(|k| urlfilter.get(&k).map(|fv| (k, fv.values.clone())))
        .collect();

    urlsbuilder(&ordered_pairs)
}

/// Resolves filter combinations from the gRPC request map first, falling back to config `urlfilter`.
pub fn resolve_url_filters(
    grpc_filter: HashMap<String, FilterValues>,
    config_filter: Option<&Vec<(String, Vec<String>)>>,
) -> Vec<String> {
    if !grpc_filter.is_empty() {
        filterbuilder(grpc_filter)
    } else if let Some(cfg_filters) = config_filter {
        urlsbuilder(cfg_filters)
    } else {
        vec![String::new()]
    }
}

/// Combines a base endpoint URL (which may already contain `?q=...`) with a generated filter expression.
pub fn build_query_url(geturl: &str, filter_expr: &str) -> String {
    let trimmed_base = geturl.trim();
    if filter_expr.is_empty() {
        if trimmed_base.contains("?q=") {
            trimmed_base
                .trim_end_matches("+AND+")
                .trim_end_matches("AND+")
                .trim_end_matches("+AND")
                .trim_end_matches("AND")
                .trim()
                .to_string()
        } else {
            trimmed_base.to_string()
        }
    } else if trimmed_base.contains("?q=") {
        if trimmed_base.ends_with("AND")
            || trimmed_base.ends_with("AND+")
            || trimmed_base.ends_with("+AND+")
            || trimmed_base.ends_with("+AND")
        {
            if geturl.ends_with(' ') || trimmed_base.ends_with('+') {
                format!("{geturl}{filter_expr}")
            } else {
                format!("{trimmed_base} {filter_expr}")
            }
        } else {
            format!("{trimmed_base} AND {filter_expr}")
        }
    } else {
        format!("{trimmed_base}?q={filter_expr}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlsbuilder_expands_combined_filters() {
        let filters = vec![
            (
                "aa::bb::cc".to_string(),
                vec!["AA".to_string(), "BB".to_string(), "CC".to_string()],
            ),
            (
                "ggggg".to_string(),
                vec!["aa".to_string(), "bb".to_string(), "cc".to_string()],
            ),
        ];

        let expanded = urlsbuilder(&filters);
        assert_eq!(expanded.len(), 9);
        assert_eq!(expanded[0], "aa::bb::cc+eq+AA AND ggggg+eq+aa");
        assert_eq!(expanded[1], "aa::bb::cc+eq+AA AND ggggg+eq+bb");
        assert_eq!(expanded[3], "aa::bb::cc+eq+BB AND ggggg+eq+aa");
        assert_eq!(expanded[4], "aa::bb::cc+eq+BB AND ggggg+eq+bb");
        assert_eq!(expanded[8], "aa::bb::cc+eq+CC AND ggggg+eq+cc");
    }

    #[test]
    fn filterbuilder_test() {
        let fi = HashMap::from([
            (
                "userid".to_string(),
                FilterValues {
                    values: vec!["user1".to_string(), "user2".to_string()],
                },
            ),
            (
                "status".to_string(),
                FilterValues {
                    values: vec!["active".to_string()],
                },
            ),
            (
                "role".to_string(),
                FilterValues {
                    values: vec!["admin".to_string(), "user".to_string()],
                },
            ),
        ]);
        let result = filterbuilder(fi);
        assert_eq!(result.len(), 4);
        assert!(
            result.contains(&"role+eq+admin AND status+eq+active AND userid+eq+user1".to_string())
        );
        assert!(
            result.contains(&"role+eq+user AND status+eq+active AND userid+eq+user2".to_string())
        );
    }

    #[test]
    fn resolve_url_filters_falls_back_to_config() {
        let cfg = vec![(
            "Objects.Name".to_string(),
            vec!["AAccount".to_string(), "BAccount".to_string()],
        )];
        let result = resolve_url_filters(HashMap::new(), Some(&cfg));
        assert_eq!(
            result,
            vec![
                "Objects.Name+eq+AAccount".to_string(),
                "Objects.Name+eq+BAccount".to_string()
            ]
        );
    }

    #[test]
    fn build_query_url_handles_existing_query_and_empty_filter() {
        assert_eq!(
            build_query_url(
                "https://oim.example.com/iam/governance/selfservice/api/v1/provtasks",
                "Objects.Name+eq+Payroll"
            ),
            "https://oim.example.com/iam/governance/selfservice/api/v1/provtasks?q=Objects.Name+eq+Payroll"
        );
        assert_eq!(
            build_query_url(
                "http://127.0.0.1:8000/users/te?q=123+eq+123 AND ",
                "aa::bb::cc+eq+AA"
            ),
            "http://127.0.0.1:8000/users/te?q=123+eq+123 AND aa::bb::cc+eq+AA"
        );
        assert_eq!(
            build_query_url("http://127.0.0.1:8000/users/te?q=123+eq+123 AND ", ""),
            "http://127.0.0.1:8000/users/te?q=123+eq+123"
        );
    }
}
