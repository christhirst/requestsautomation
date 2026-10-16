use crate::{
    error::CliError,
    grpcserver::proto,
    types::{Action, ProvActionPayload},
};

pub fn action_mapper(req: &proto::ProvActionRequest) -> Result<ProvActionPayload, CliError> {
    let action = Action::try_from(req.action)
        .map_err(|code| CliError::InvalidAction(format!("Unknown action code: {code}")))?;

    Ok(ProvActionPayload {
        action: action.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_mapper_maps_retry_and_manual_complete() {
        let retry_req = proto::ProvActionRequest {
            action: proto::ActionType::Retry as i32,
            mode: String::new(),
            task_ids: vec![],
        };
        let mapped_retry = action_mapper(&retry_req).unwrap();
        assert_eq!(mapped_retry.action, "retry");

        let mc_req = proto::ProvActionRequest {
            action: proto::ActionType::ManualComplete as i32,
            mode: String::new(),
            task_ids: vec![],
        };
        let mapped_mc = action_mapper(&mc_req).unwrap();
        assert_eq!(mapped_mc.action, "manualComplete");
    }

    #[test]
    fn action_mapper_rejects_invalid_action() {
        let invalid_req = proto::ProvActionRequest {
            action: 42,
            mode: String::new(),
            task_ids: vec![],
        };
        assert!(action_mapper(&invalid_req).is_err());
    }
}
