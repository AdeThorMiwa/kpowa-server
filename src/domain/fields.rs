use rand::{distributions::Uniform, prelude::Distribution};
use serde::{Deserialize, Serialize};
use std::fmt::Display;

use super::model::DbUser;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Username(String);

impl Username {
    pub fn inner(&self) -> String {
        self.0.to_owned()
    }
}

impl From<String> for Username {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl AsRef<str> for Username {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

impl Display for Username {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct InviteCode(String);

impl InviteCode {
    pub fn new(username: &str) -> Self {
        Self(format!(
            "{}{}",
            &username[..=2],
            Self::generate_invite_code_digit()
        ))
    }

    pub fn inner(&self) -> String {
        self.0.to_owned()
    }

    fn generate_invite_code_digit() -> String {
        let mut rng = rand::thread_rng();
        let uni_sample = Uniform::from(1001..=9999);
        let code = uni_sample.sample(&mut rng);
        code.to_string()
    }
}

impl From<String> for InviteCode {
    fn from(value: String) -> Self {
        Self(value)
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub username: Username,
    pub invite_code: InviteCode,
    pub referred_by: Option<Username>,
    pub referrals: i64,
}

impl From<DbUser> for User {
    fn from(value: DbUser) -> Self {
        Self {
            username: value.username.into(),
            invite_code: value.invite_code.into(),
            referred_by: value.referred_by.map(|r| Username::from(r)),
            referrals: value.referrals.unwrap_or(0),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub iss: String,
    pub exp: usize,
}
