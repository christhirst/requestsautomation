use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[allow(dead_code)]
#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resp {
    pub links: Vec<Link>,
    pub id: String,
    pub status: String,
}

#[derive(Default, Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    #[serde(default)]
    pub links: Vec<Link>,
    #[serde(default)]
    pub count: i64,
    #[serde(default)]
    pub has_more: bool,
    #[serde(default)]
    pub total_result: i64,
    pub accounts: Option<Vec<Account>>,
    pub tasks: Option<Vec<Task>>,
}

impl fmt::Display for Root {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.count)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Roots {
    Root(Root),
}

#[derive(Default, Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Link {
    pub rel: String,
    pub href: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    #[serde(default)]
    pub links: Vec<Link>,
    #[serde(default)]
    pub fields: Vec<Field>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub name: String,
    pub value: Value,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    #[serde(default)]
    pub links: Vec<Link>,
    pub id: String,
    pub name: String,
    pub user_id: String,
    pub app_instance_id: String,
    pub request_id: String,
    #[serde(default)]
    pub fields: Vec<Field>,
}
