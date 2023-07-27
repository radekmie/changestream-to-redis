trait IntoMeteorEJSON {
    fn into_meteor_ejson(self) -> serde_json::Value;
}

impl IntoMeteorEJSON for bson::Bson {
    fn into_meteor_ejson(self) -> serde_json::Value {
        match self {
            // Meteor EJSON serialization.
            bson::Bson::Binary(v) => {
                use base64::engine::{general_purpose::STANDARD, Engine};
                serde_json::json!({ "$binary": STANDARD.encode(v.bytes)})
            }
            bson::Bson::DateTime(v) => serde_json::json!({ "$date": v.timestamp_millis() }),
            bson::Bson::Decimal128(v) => {
                serde_json::json!({ "$type": "Decimal", "$value": v.to_string() })
            }
            bson::Bson::Double(v) if v.is_infinite() => {
                serde_json::json!({ "$InfNaN": v.signum() })
            }
            bson::Bson::Double(v) if v.is_nan() => serde_json::json!({ "$InfNan": 0 }),
            bson::Bson::ObjectId(v) => serde_json::json!({ "$type": "oid", "$value": v.to_hex() }),
            bson::Bson::RegularExpression(v) => {
                serde_json::json!({ "$regexp": v.pattern, "$flags": v.options })
            }

            // Standard JSON serialization.
            bson::Bson::Array(v) => serde_json::Value::Array(
                v.into_iter()
                    .map(IntoMeteorEJSON::into_meteor_ejson)
                    .collect(),
            ),
            bson::Bson::Boolean(v) => serde_json::json!(v),
            bson::Bson::Document(v) => serde_json::Value::Object(
                v.into_iter()
                    .map(|(k, v)| (k, v.into_meteor_ejson()))
                    .collect(),
            ),
            bson::Bson::Double(v) => serde_json::json!(v),
            bson::Bson::Int32(v) => serde_json::json!(v),
            bson::Bson::Int64(v) => serde_json::json!(v),
            bson::Bson::Null => serde_json::Value::Null,
            bson::Bson::String(v) => serde_json::json!(v),

            // Replace everything else with `null`s.
            v => {
                println!("Unrecognized BSON value found: {}", v);
                serde_json::Value::Null
            }
        }
    }
}

#[derive(serde::Deserialize)]
struct Event {
    ns: String,
    id: String,
    op: bson::Bson,
}

struct Mongo {
    change_stream: mongodb::change_stream::ChangeStream<Event>,
}

impl Mongo {
    async fn new() -> Result<Self, mongodb::error::Error> {
        // By default, only the `_id` field is present, matching the `oplogtoredis` behavior.
        // However, thanks to the `fullDocument` option in change streams, we can get the entire
        // document at the same time. Both `updateLookup` and `required` variants incur an
        // additional performance cost, but it's most likely less than an additional query coming
        // from the outside. Having the whole document in Redis allows us to use the
        // `protectAgainstRaceConditions: false` in the app, skipping the database call entirely.
        let full_document = match std::env::var("FULL_DOCUMENT") {
            Ok(flag) => match flag.as_str() {
                "required" => Some(mongodb::options::FullDocumentType::Required),
                "updateLookup" => Some(mongodb::options::FullDocumentType::UpdateLookup),
                "whenAvailable" => Some(mongodb::options::FullDocumentType::WhenAvailable),
                _ => Some(mongodb::options::FullDocumentType::Other(flag)),
            },
            _ => None,
        };

        let pipeline = [
            bson::doc! {"$match": {
                "documentKey._id": {"$type": ["objectId", "string"]},
                "operationType": {"$in": ["delete", "insert", "replace", "update"]},
            }},
            bson::doc! {"$project": {
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
                    "d": if full_document.is_some() {
                        bson::doc! {"$ifNull": ["$fullDocument", {"_id": "$documentKey._id"}]}
                    } else {
                        bson::doc! {"_id": "$documentKey._id"}
                    },
                    "f": []
                }
            }},
        ];

        let options = mongodb::options::ChangeStreamOptions::builder()
            .full_document(full_document)
            .build();

        let uri = std::env::var("MONGO_URL").expect("MONGO_URL is required");
        let change_stream = mongodb::Client::with_uri_str(uri)
            .await?
            .default_database()
            .expect("MONGO_URL is missing default database")
            .watch(pipeline, options)
            .await?
            .with_type();
        Ok(Self { change_stream })
    }

    async fn next(&mut self) -> Result<Option<Event>, mongodb::error::Error> {
        use futures_util::StreamExt;
        self.change_stream.next().await.transpose()
    }
}

struct Redis {
    connection: redis::aio::Connection,
    script: redis::Script,
}

impl Redis {
    async fn new() -> Result<Self, redis::RedisError> {
        let connection =
            redis::Client::open(std::env::var("REDIS_URL").expect("REDIS_URL is required"))?
                .get_async_connection()
                .await?;
        let script = redis::Script::new(
            r#"redis.call("PUBLISH", KEYS[1], ARGV[1])
               redis.call("PUBLISH", KEYS[1] .. '::' .. KEYS[2], ARGV[1])"#,
        );
        Ok(Self { connection, script })
    }

    async fn publish(&mut self, event: Event) -> Result<(), redis::RedisError> {
        println!(
            "{}::{} {}",
            event.ns,
            event.id,
            event.op.clone().into_meteor_ejson()
        );
        self.script
            .key(event.ns)
            .key(event.id)
            .arg(event.op.into_meteor_ejson().to_string())
            .invoke_async(&mut self.connection)
            .await
    }
}

#[tokio::main]
async fn main() {
    let mut mongo = Mongo::new().await.unwrap();
    let mut redis = Redis::new().await.unwrap();
    let (sender, mut receiver) = tokio::sync::mpsc::channel(1024);

    tokio::spawn(async move {
        while let Some(event) = mongo.next().await.unwrap() {
            sender.send(event).await.unwrap();
        }
    });

    while let Some(event) = receiver.recv().await {
        redis.publish(event).await.unwrap();
    }
}
