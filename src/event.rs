use bson::{Bson, Timestamp};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Event {
    pub ct: Timestamp,
    #[serde(rename = "_id")]
    pub ev: Bson,
    pub ns: String,
    pub id: String,
    pub op: Bson,
}
