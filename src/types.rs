use serde::Serialize;
use strum_macros::Display;

/* use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub rel: String,
    pub href: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resp {
    pub links: Vec<Link>,
    pub id: String,
    pub status: String,
}
 */
#[derive(Serialize, Debug, Clone)]
pub struct ProvAcionRequest {
    pub action: String,
}
#[derive(Clone, Copy, Display)]
// If we don't care about inner capitals, we don't need to set `serialize_all`
// and can leave parenthesis empty.
#[strum(serialize_all = "lowercase")]
pub enum Action {
    Retry,
    ManualComplete,
}

impl TryFrom<i32> for Action {
    type Error = ();
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            x if x == Action::Retry as i32 => Ok(Action::Retry),
            x if x == Action::ManualComplete as i32 => Ok(Action::ManualComplete),
            _ => Err(()),
        }
    }
}
