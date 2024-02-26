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
    let mut data = fetchdata(&client, url, username, password).await?;
    println!("Entires in Backend: {}", data.count);
    let gotcount = data.count;
    let mut alltasks: Vec<Task> = vec![];
    alltasks.append(&mut data.tasks);

    let mut more = data.has_more;

    let mut count = 0;
    let link = data.links.get(3);
    let next = link.ok_or(CliError::EntityNotFound { entity: "", id: 1 })?;
    let mut uri = "".to_owned();
    while more && next.rel == "next" && count < fetched {
        if next.rel == "next" {
            uri = next.href.clone();
        }
        let mut data = fetchdata(&client, &url, username, password).await?;
        alltasks.append(&mut data.tasks);
        more = data.has_more;

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
