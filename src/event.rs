use bson::Bson;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Event {
    pub _id: Bson,
    pub ns: String,
    pub id: String,
    pub op: Bson,
}
