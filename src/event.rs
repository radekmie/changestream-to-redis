use crate::ejson::Ejson;
use bson::{Bson, Timestamp};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Event {
    #[serde(rename = "c")]
    pub collection: String,
    #[serde(rename = "d")]
    pub db: String,
    #[serde(rename = "i")]
    pub document_id: String,
    #[expect(clippy::struct_field_names)]
    #[serde(rename = "_id")]
    pub event_id: Bson,
    #[serde(rename = "n")]
    pub namespaces: String,
    #[serde(rename = "o")]
    pub operation: Bson,
    #[serde(rename = "t")]
    pub timestamp: Timestamp,
}

impl Event {
    pub fn debug(&self) {
        let Self {
            collection,
            db,
            document_id,
            namespaces,
            operation,
            ..
        } = self;

        let ejson = operation.clone().into_ejson();
        println!("{db}.{collection} {ejson}");
        println!("{db}.{collection}::{document_id} {ejson}");
        for namespace in namespaces.split(',').filter(|x| !x.is_empty()) {
            println!("{db}.{namespace}::{collection} {ejson}");
        }
    }
}
