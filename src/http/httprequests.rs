use std::time::Duration;

use reqwest::{
    header::{ACCEPT, CONTENT_TYPE},
    Client, Response,
};
use tracing::{debug, info};

use crate::{
    error::CliError,
    model::{Root, Roots, Task},
    types::ProvAcionRequest,
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
    let mut url: String = url.to_string();

    //let mut next_link: String;
    while more && count < fetched {
        let dat = Root::default();
        debug!("Fetching data from: {}", url);
        let data = fetchdata(client, &mut url, username, password, dat).await?;
        debug!("Entires in Backend: {:?}", data);
        match data {
            Roots::Root(d) => {
                //data = Roots::RootAccount(d);
                more = fetchdatass(d, &mut alltasks, &mut url);
                //let gotcount = count;
            } //Roots::RootAccount(d) => data = Roots::RootAccount(d),
        }

        count += 1;
    }

    Ok(alltasks)
}

fn fetchdatass(data: Root, alltasks: &mut Vec<Task>, url: &mut String) -> (bool) {
    alltasks.append(&mut data.tasks.unwrap());
    let more = data.has_more;
    //let mut next_link: String;
    for l in data.links {
        if l.rel == "next" {
            *url = l.href.to_owned();
        }
    }
    info!("{}", data.count / data.total_result);
    //println!("{}", data.count / data.total_result);
    more
    //todo!()
}

async fn fetchdata<T>(
    client: &Client,
    url: &str,
    username: &str,
    password: &str,
    mut t: T,
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
    body: ProvAcionRequest,
    username: &str,
    password: &str,
) -> Result<Response, CliError> {
    let response = client
        .put(url)
        .json(&body)
        .header(CONTENT_TYPE, "application/json")
        .header("X-Requested-By", "rust")
        .basic_auth(username, Some(password))
        .timeout(Duration::from_secs(3))
        .send()
        .await?;
    Ok(response)
}

// build url like:
// /iam/governance/selfservice/api/v1/accounts/account?userid={userKey}
pub fn urlsbuilder(_urlsnippets: &str, urlfilter: &Vec<(String, Vec<String>)>) -> Vec<String> {
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
