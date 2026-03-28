use crate::{
    error::CliError,
    grpcserver::proto,
    types::{Action, ProvAcionRequest},
};

pub fn action_mapper(
    re: tonic::Request<proto::ProvAcionRequest>,
) -> Result<ProvAcionRequest, CliError> {
    let action = match &re.get_ref().action.try_into() {
        Ok(Action::Retry) => Some(Action::Retry.to_string()),
        Ok(Action::ManualComplete) => Some(Action::ManualComplete.to_string()),
        Err(_) => {
            panic!("Unknown action");
        }
    }
    .ok_or(CliError::EntityNotFound { entity: "", id: 1 })?;

    Ok(ProvAcionRequest { action: action })
}

/* pub async fn getheaders(
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
} */
