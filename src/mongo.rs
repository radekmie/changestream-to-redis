use crate::{config::Config, event::Event};
use bson::doc;
use futures_util::StreamExt;
use mongodb::{
    change_stream::ChangeStream,
    error::Error,
    options::{ChangeStreamOptions, FullDocumentBeforeChangeType, FullDocumentType},
    Client,
};

pub struct Mongo {
    stream1: ChangeStream<Event>,
    stream2: Option<ChangeStream<Event>>,
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

    /// Polls the next `Event` from either of change streams.
    pub async fn next(&mut self) -> Result<Option<Event>, Error> {
        let Self { stream1, stream2 } = self;
        match stream2 {
            None => stream1.next().await.transpose(),
            Some(stream2) => tokio::select! {
                biased;
                event = stream1.next() => event.transpose(),
                event = stream2.next() => event.transpose(),
            },
        }
    }
}

async fn create_change_stream(
    client: &Client,
    config: &Config,
    primary: bool,
) -> Result<ChangeStream<Event>, Error> {
    // Only the primary stream will receive full documents, and only if the `full_document` is set.
    // However, as `namespace_fields` requires the field values to work, it implies `full_document`
    // flag set.
    let full_document = primary
        .then(|| config.full_document.clone())
        .flatten()
        .or_else(|| {
            config
                .namespaces
                .is_some()
                .then_some(FullDocumentType::UpdateLookup)
        });

    client
        .default_database()
        .expect("MONGO_URL is missing default database")
        .watch(
            create_pipeline(config, primary),
            ChangeStreamOptions::builder()
                .full_document(full_document)
                .full_document_before_change(
                    config
                        .namespaces
                        .is_some()
                        .then_some(FullDocumentBeforeChangeType::WhenAvailable),
                )
                .build(),
        )
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
        document = doc! {"$ifNull": ["$fullDocument", {"$ifNull": ["$fullDocumentBeforeChange", {"_id": document}]}]};
    }

    // Comma separated list of namespaces (including array flattening).
    let ns = config.namespaces.iter().flatten().fold(
        doc! {"$literal": ""},
        |initial_value, (collection, field)| {
            let namespace = format!("{field}::");
            let next_value = format!("$fullDocument.{field}");
            let prev_value = format!("$fullDocumentBeforeChange.{field}");
            doc! {"$reduce": {
                "input": {"$cond": {
                    "if": {"$eq": ["$ns.coll", collection]},
                    "then": {"$let": {
                        "vars": {"v": {"$ifNull": [next_value, {"$ifNull": [prev_value, []]}]}},
                        "in": {"$cond": {
                            "if": {"$isArray": "$$v"},
                            "then": "$$v",
                            "else": ["$$v"]
                        }}
                    }},
                    "else": []
                }},
                "initialValue": initial_value,
                "in": {"$concat": ["$$value", ",", namespace, {"$toString": "$$this"}]}
            }}
        },
    );

    [
        doc! {"$match": query},
        doc! {"$project": {
            "c": "$ns.coll",
            "d": "$ns.db",
            // The ID is stringified to support `ObjectID`s.
            "i": {"$toString": "$documentKey._id"},
            "n": ns,
            "o": {
                "e": {"$switch": {
                    "branches": [
                        {"case": {"$eq": ["$operationType", "delete"]}, "then": "r"},
                        {"case": {"$eq": ["$operationType", "insert"]}, "then": "i"}
                    ],
                    "default": "u"
                }},
                "d": document,
                "f": []
            },
            "t": "$clusterTime"
        }},
    ]
}
