use crate::{config::Config, event::Event};
use bson::doc;
use futures_util::StreamExt;
use mongodb::{change_stream::ChangeStream, error::Error, options::ChangeStreamOptions, Client};

pub struct Mongo {
    change_stream: ChangeStream<Event>,
}

impl Mongo {
    pub async fn new(config: &Config) -> Result<Self, Error> {
        let pipeline = [
            doc! {"$match": {
                "documentKey._id": {"$type": ["objectId", "string"]},
                "operationType": {"$in": ["delete", "insert", "replace", "update"]},
            }},
            doc! {"$project": {
                // The ID is stringified to support `ObjectID`s.
                "id": {"$toString": "$documentKey._id"},
                // All changes are published in their collection's channel and a subscope with
                // their ID. We match the `cultofcoders:redis-oplog` format here.
                "ns": {"$concat": ["$ns.db", ".", "$ns.coll"]},
                "op": {
                    "e": {"$switch": {
                        "branches": [
                            {"case": {"$eq": ["$operationType", "delete"]}, "then": "r"},
                            {"case": {"$eq": ["$operationType", "insert"]}, "then": "i"}
                        ],
                        "default": "u"
                    }},
                    "d": if config.full_document.is_some() {
                        doc! {"$ifNull": ["$fullDocument", {"_id": "$documentKey._id"}]}
                    } else {
                        doc! {"_id": "$documentKey._id"}
                    },
                    "f": []
                }
            }},
        ];

        let options = ChangeStreamOptions::builder()
            .full_document(config.full_document.clone())
            .build();

        let change_stream = Client::with_uri_str(config.mongo_url.as_str())
            .await?
            .default_database()
            .expect("MONGO_URL is missing default database")
            .watch(pipeline, options)
            .await?
            .with_type();

        println!("Mongo connection initialized.");
        Ok(Self { change_stream })
    }

    pub async fn next(&mut self) -> Result<Option<Event>, Error> {
        self.change_stream.next().await.transpose()
    }
}
