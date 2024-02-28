use reqwest::{
    header::{ACCEPT, CONTENT_TYPE},
    Client,
};

use crate::{CliError, Root, Task};

pub async fn get_data(
    client: &Client,
    url: &str,
    username: &str,
    password: &str,
    fetched: u32,
) -> Result<Vec<Task>, CliError> {
    let mut data = fetchdata(client, url, username, password).await?;
    println!("Entires in Backend: {}", data.count);
    let gotcount = data.count;
    let mut alltasks: Vec<Task> = vec![];
    alltasks.append(&mut data.tasks);

    let mut more = data.has_more;

    let mut count = 0;
    let mut next_link = "".to_owned();

    for l in data.links {
        if l.rel == "next" {
            next_link = l.href;
        }
    }

    //let link = data.links.get(3);
    //let next = link.ok_or(CliError::EntityNotFound { entity: "", id: 1 })?;

    while more && count < fetched {
        let mut data = fetchdata(client, &next_link, username, password).await?;
        alltasks.append(&mut data.tasks);
        more = data.has_more;

        for l in data.links {
            if l.rel == "next" {
                next_link = l.href;
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
