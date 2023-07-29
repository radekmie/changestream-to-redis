use bson::Bson;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Event {
    #[serde(rename = "_id")]
    pub ev: Bson,
    pub ns: String,
    pub id: String,
    pub op: Bson,
}
