use reqwest::{
    header::{ACCEPT, CONTENT_TYPE},
    Client,
};

use crate::{CliError, Root, Task};

pub async fn get_data(
    url: &str,
    username: &str,
    password: &str,
    fetched: u32,
) -> Result<Vec<Task>, CliError> {
    let client = reqwest::Client::new();
    let data = fetchdata(&client, url, username, password).await?;

    let mut alltasks: Vec<Task> = vec![];

    let mut count = 0;

    let link = data.links.get(3);
    let next = link.ok_or(CliError::EntityNotFound { entity: "", id: 1 })?;
    let mut uri = "".to_owned();
    while data.has_more && count < fetched && next.rel == "next" {
        if next.rel == "next" {
            uri = next.href.clone();
        }
        //data = fetchdata(&client, &ouri, username, password).await?.tasks;
        alltasks.append(&mut fetchdata(&client, &uri, username, password).await?.tasks);

        count += 1;
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
