use std::time::Duration;

use reqwest::{
    header::{ACCEPT, CONTENT_TYPE},
    Client, Response,
};

use crate::{CliError, Root, Roots, Task};

pub async fn get_data(
    client: &Client,
    mut url: &str,
    username: &str,
    password: &str,
    fetched: u32,
) -> Result<Vec<Task>, CliError> {
    let mut alltasks: Vec<Task> = vec![];

    let mut more = true;
    let mut count = 0;

    let mut next_link: String;
    while more && count < fetched {
        let mut data = fetchdata(client, url, username, password).await?;

        println!("Entires in Backend: {}", data.count);
        let gotcount = data.count;

        alltasks.append(&mut data.tasks);
        more = data.has_more;

        for l in data.links {
            if l.rel == "next" {
                next_link = l.href.to_owned();
                url = next_link.as_ref();
            }
        }

        count += 1;
        println!("{}", count as f32 / gotcount as f32);
    }

    Ok(alltasks)
}

async fn fetchdata(
    client: &Client,
    url: &str,
    username: &str,
    password: &str,
) -> Result<Root, CliError> {
    let response = client
        .get(url)
        .header(CONTENT_TYPE, "application/json")
        .header(ACCEPT, "application/json")
        .basic_auth(username, Some(password))
        .send()
        .await?;
    let json: Root = response.json().await?;
    Ok(json)
}

pub async fn retrycall(
    client: &Client,
    url: String,
    body: String,
    username: String,
    password: String,
) -> Result<Response, CliError> {
    let response = client
        .put(url)
        .body(body)
        .header(CONTENT_TYPE, "application/json")
        .header("X-Requested-By", "rust")
        .basic_auth(username, Some(password))
        .timeout(Duration::from_secs(3))
        .send()
        .await?;
    Ok(response)
}

// /iam/governance/selfservice/api/v1/accounts/account?userid={userKey}
// /iam/governance/selfservice/api/v1/accounts/{accountid}

pub fn urlsbuilder(urlsnippets: &str, urlfilter: &Vec<(String, Vec<String>)>) -> Vec<String> {
    let mut uri: Vec<Vec<String>> = Vec::new();
    for (url, filters) in urlfilter {
        let mut tup: Vec<String> = Vec::new();
        for filter in filters {
            let ent = format!("{}+eq+{}", url, filter);
            tup.push(ent)
            //tup[i] = ent;
        }
        uri.push(tup);
    }
    let mut combined = Vec::new();
    //for i in &uri {
    if uri.len() > 1 {
        for item1 in &uri[0] {
            for item2 in &uri[1] {
                combined.push(format!("{} AND {}", item1, item2));
            }
        }
    } else {
        combined = uri.get(0).unwrap().clone();
    }
    combined
}
