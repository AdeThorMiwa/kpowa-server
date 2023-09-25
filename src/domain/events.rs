use super::fields::{User, Username};
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct NewReferralEvent {
    pub referrer: Username,
    pub referred_user: Username,
}

#[derive(Serialize, Clone)]
#[serde(tag = "type", content = "data")]
pub enum AppEvent {
    NewLogin(User),
    NewRegister(User),
    NewReferral(NewReferralEvent),
}
