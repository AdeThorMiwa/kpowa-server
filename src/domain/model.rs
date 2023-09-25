use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Serialize, Deserialize, FromRow)]
pub struct DbUser {
    pub uid: Uuid,
    pub(crate) username: String,
    pub(crate) invite_code: String,
    pub(crate) referred_by: Option<String>,
    pub(crate) referrals: Option<i64>,
    pub(crate) created_on: OffsetDateTime,
}
