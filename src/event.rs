#![allow(clippy::module_name_repetitions)]

use bson::{Bson, Timestamp};
use serde::Deserialize;
use serde_json::Value;

use crate::ejson::Ejson;

#[derive(Deserialize)]
pub struct ChangeStreamEvent {
    pub ct: Timestamp,
    #[serde(rename = "_id")]
    pub ev: Bson,
    pub ns: String,
    pub id: String,
    pub op: Bson,
}

pub struct RedisEvent {
    pub ct: Timestamp,
    pub ev: Bson,
    pub ns: String,
    pub id: String,
    pub op: Value,
}

impl From<ChangeStreamEvent> for RedisEvent {
    fn from(event: ChangeStreamEvent) -> Self {
        Self {
            ct: event.ct,
            ev: event.ev,
            ns: event.ns,
            id: event.id,
            op: event.op.into_ejson(),
        }
    }
}
