use serde::Serialize;
use strum_macros::Display;

#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
pub struct ProvActionPayload {
    pub action: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Display)]
#[strum(serialize_all = "camelCase")]
pub enum Action {
    Retry,
    ManualComplete,
}

impl TryFrom<i32> for Action {
    type Error = i32;
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Action::Retry),
            1 => Ok(Action::ManualComplete),
            other => Err(other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_serialization_matches_oim_api() {
        assert_eq!(Action::Retry.to_string(), "retry");
        assert_eq!(Action::ManualComplete.to_string(), "manualComplete");
    }

    #[test]
    fn action_try_from_i32() {
        assert_eq!(Action::try_from(0), Ok(Action::Retry));
        assert_eq!(Action::try_from(1), Ok(Action::ManualComplete));
        assert_eq!(Action::try_from(99), Err(99));
    }
}
