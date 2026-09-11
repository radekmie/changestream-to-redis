use crate::{config::Config, event::Event, tokens::ResumeTokens};
use bson::doc;
use futures_util::StreamExt;
use mongodb::{
    action::{Action, Watch},
    change_stream::{event::ResumeToken, ChangeStream},
    error::{Error, ErrorKind},
    options::{FullDocumentBeforeChangeType, FullDocumentType},
    Client,
};

pub struct Mongo {
    stream1: ChangeStream<Event>,
    stream2: Option<ChangeStream<Event>>,
}

impl Mongo {
    pub async fn new(config: &Config, tokens: &ResumeTokens) -> Result<Self, Error> {
        let client = Client::with_uri_str(config.mongo_url.as_str()).await?;
        let stream1 = resume_change_stream(&client, config, true, tokens.primary()).await?;
        let stream2 = match &config.full_document_collections {
            None => None,
            Some(_) => {
                Some(resume_change_stream(&client, config, false, tokens.secondary()).await?)
            }
        };

        println!("Mongo connection initialized.");
        Ok(Self { stream1, stream2 })
    }

    /// Polls the next `Event` from either of change streams.
    pub async fn next(&mut self) -> Result<Option<Event>, Error> {
        let Self { stream1, stream2 } = self;
        let event = match stream2 {
            None => stream1.next().await.transpose()?.map(|event| Event {
                is_primary: true,
                ..event
            }),
            Some(stream2) => tokio::select! {
                biased;
                event = stream1.next() => event.transpose()?.map(|event| Event {
                    is_primary: true,
                    ..event
                }),
                event = stream2.next() => event.transpose()?,
            },
        };

        Ok(event)
    }
}

/// Starts a change stream at `token`, falling back to the current position if the token is too old to resume from.
async fn resume_change_stream(
    client: &Client,
    config: &Config,
    primary: bool,
    token: Option<&ResumeToken>,
) -> Result<ChangeStream<Event>, Error> {
    // `ChangeStreamFatalError` and `ChangeStreamHistoryLost`. Both mean the stored token is no longer
    // in the oplog, i.e., we were down for longer than the oplog window.
    const UNRESUMABLE_ERROR_CODES: [i32; 2] = [280, 286];

    match create_change_stream(client, config, primary, token.cloned()).await {
        Ok(change_stream) => Ok(change_stream),
        Err(error) => match error.kind.as_ref() {
            ErrorKind::Command(command) if UNRESUMABLE_ERROR_CODES.contains(&command.code) => {
                eprintln!(
                    "Cannot resume the change stream ({}, code {}). Events since the stored resume token were lost. Starting from the current position.",
                    command.code_name,
                    command.code
                );

                create_change_stream(client, config, primary, None).await
            }
            _ => Err(error),
        },
    }
}

async fn create_change_stream(
    client: &Client,
    config: &Config,
    primary: bool,
    token: Option<ResumeToken>,
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
        .watch()
        .pipeline(create_pipeline(config, primary))
        .optional(config.mongo_batch_size, Watch::batch_size)
        .optional(full_document, Watch::full_document)
        .optional(
            config
                .namespaces
                .is_some()
                .then_some(FullDocumentBeforeChangeType::WhenAvailable),
            Watch::full_document_before_change,
        )
        .optional(config.mongo_max_await_time, Watch::max_await_time)
        .optional(token, Watch::start_after)
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
                        "in": {"$switch": {
                            "branches": [
                                // Arrays are flattened.
                                {
                                    "case": {"$eq": [{"$type": "$$v"}, "array"]},
                                    "then": "$$v"
                                },
                                // Objects are mapped to their keys.
                                {
                                    "case": {"$eq": [{"$type": "$$v"}, "object"]},
                                    "then": {"$map": {
                                        "input": {"$objectToArray": "$$v"},
                                        "in": "$$this.k"
                                    }}
                                },
                            ],
                            // Other values are left as-is.
                            "default": ["$$v"]
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
