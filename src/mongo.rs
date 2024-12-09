use crate::{
    config::Config,
    event::{ChangeStreamEvent, RedisEvent},
};
use bson::doc;
use futures_util::StreamExt;
use mongodb::{change_stream::ChangeStream, error::Error, options::ChangeStreamOptions, Client};

pub struct Mongo {
    stream1: ChangeStream<ChangeStreamEvent>,
    stream2: Option<ChangeStream<ChangeStreamEvent>>,
}

impl Mongo {
    pub async fn new(config: &Config) -> Result<Self, Error> {
        let client = Client::with_uri_str(config.mongo_url.as_str()).await?;
        let stream1 = create_change_stream(&client, config, true).await?;
        let stream2 = match &config.full_document_collections {
            None => None,
            Some(_) => Some(create_change_stream(&client, config, false).await?),
        };

        println!("Mongo connection initialized.");
        Ok(Self { stream1, stream2 })
    }

    /// Polls the next `RedisEvent` from either of change streams.
    pub async fn next(&mut self) -> Result<Option<RedisEvent>, Error> {
        let Self { stream1, stream2 } = self;

        let to_redis_event = |e: Result<Option<ChangeStreamEvent>, Error>| {
            e.map(|e| e.map(std::convert::Into::into))
        };

        match stream2 {
            None => to_redis_event(stream1.next().await.transpose()),
            Some(stream2) => tokio::select! {
                biased;
                event = stream1.next() => to_redis_event(event.transpose()),
                event = stream2.next() => to_redis_event(event.transpose()),
            },
        }
    }
}

async fn create_change_stream(
    client: &Client,
    config: &Config,
    primary: bool,
) -> Result<ChangeStream<ChangeStreamEvent>, Error> {
    // Only the primary stream will receive full documents, and only if the `full_document` is set.
    let options = primary.then(|| {
        ChangeStreamOptions::builder()
            .full_document(config.full_document.clone())
            .build()
    });

    client
        .default_database()
        .expect("MONGO_URL is missing default database")
        .watch(create_pipeline(config, primary), options)
        .await
        .map(ChangeStream::with_type)
}

fn create_pipeline(config: &Config, primary: bool) -> [bson::Document; 2] {
    // Filter events that...
    // 1. We actually can process, i.e., their `_id` is handled in `cultofcoders:redis-oplog`.
    // 2. We are interested in, i.e., `cultofcoders:redis-oplog` is interested in.
    let mut query = doc! {
        "documentKey._id": {"$type": ["objectId", "string"]},
        "operationType": {"$in": ["delete", "insert", "replace", "update"]},
    };

    // 3. Match the collection filters if there's any.
    if let Some(names) = config.excluded_collections.clone() {
        query.insert("ns.coll", doc! { "$nin": names });
    }

    if let Some(names) = config.full_document_collections.clone() {
        let operator = if primary { "$in" } else { "$nin" };
        query.insert("ns.coll", doc! { operator: names });
    }

    // There are two streams -- primary and secondary. The former receives whole documents if they
    // are requested (`full_document` is set) or simply available (`full_document_collections` is
    // set; only `insert` will have it without `full_document` set).
    let mut document = doc! {"_id": "$documentKey._id"};
    if primary && (config.full_document.is_some() || config.full_document_collections.is_some()) {
        document = doc! {"$ifNull": ["$fullDocument", {"_id": document}]};
    }

    [
        doc! {"$match": query},
        doc! {"$project": {
            "ct": "$clusterTime",
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
                "d": document,
                "f": []
            }
        }},
    ]
}
