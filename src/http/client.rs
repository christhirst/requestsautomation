pub fn rest_client(timeout: u64) -> Result<reqwest::Client, tonic::Status> {
    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(timeout))
        .build()
        .map_err(|e| tonic::Status::new(tonic::Code::Internal, format!("{:?}", e)))?;

    Ok(client)
}
