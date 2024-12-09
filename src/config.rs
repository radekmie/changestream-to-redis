use mongodb::options::FullDocumentType;
use serde_json::from_str;
use std::{env::var, time::Duration, vec::Vec};

pub struct Config {
    /// If true, all events are logged before being sent to Redis.
    pub debug: bool,
    /// If present, all events are deduplicated on Redis. That allows you to
    /// deploy multiple instances of `changestream-to-redis` listening to the
    /// same MongoDB database and pushing to the same Redis database.
    pub deduplication: Option<usize>,
    /// If set, events from these collections will be ignored (i.e., won't get
    /// published to Redis). It allows you reduce `changestream-to-redis` and
    /// Redis load by ignoring write-intensive collections that don't require
    /// reactivity.
    pub excluded_collections: Option<Vec<String>>,
    /// By default, only the `_id` field is present, matching the `oplogtoredis`
    /// behavior. However, thanks to the `fullDocument` option in change
    /// streams, we can get the entire document at the same time. Both
    /// `updateLookup` and `required` variants incur an additional performance
    /// cost, but it's most likely less than an additional query coming from the
    /// outside. Having the whole document in Redis allows us to use the
    /// `protectAgainstRaceConditions: false` in the app, skipping the database
    /// call entirely. If `full_document_collections` is set, only those
    /// collections will have all of the fields included.
    pub full_document: Option<FullDocumentType>,
    /// If set, `changestream-to-redis` will create two change streams -- one
    /// for ID-only collections and one for `full_document` collections. If
    /// `full_document` is not be set, only some operations will have all of the
    /// fields (i.e., inserts).
    pub full_document_collections: Option<Vec<String>>,
    /// If set, `changestream-to-redis` will expose Prometheus metrics at this
    /// address.
    pub metrics_address: Option<String>,
    pub mongo_url: String,
    pub redis_batch_size: usize,
    pub redis_queue_size: usize,
    pub redis_url: String,
    pub redis_response_timeout: Duration,
    pub redis_connection_timeout: Duration,
    pub redis_max_delay: Option<Duration>,
    pub redis_connection_retry_count: usize,
    pub redis_publish_retry_count: usize,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            debug: var("DEBUG").is_ok(),
            deduplication: var("DEDUPLICATION")
                .ok()
                .map(|value| value.parse().unwrap()),
            excluded_collections: var("EXCLUDED_COLLECTIONS")
                .ok()
                .map(|value| value.split(',').map(ToString::to_string).collect()),
            full_document: var("FULL_DOCUMENT")
                .ok()
                .map(|value| from_str(format!("\"{value}\"").as_str()).unwrap()),
            full_document_collections: var("FULL_DOCUMENT_COLLECTIONS")
                .ok()
                .map(|value| value.split(',').map(ToString::to_string).collect()),
            metrics_address: var("METRICS_ADDRESS").ok(),
            mongo_url: var("MONGO_URL").expect("MONGO_URL is required"),
            redis_batch_size: var("REDIS_BATCH_SIZE")
                .ok()
                .map_or(1, |value| value.parse().unwrap()),
            redis_queue_size: var("REDIS_QUEUE_SIZE")
                .ok()
                .map_or(1024, |value| value.parse().unwrap()),
            redis_url: var("REDIS_URL").expect("REDIS_URL is required"),
            redis_response_timeout: Duration::from_secs(
                var("REDIS_RESPONSE_TIMEOUT_SECS")
                    .ok()
                    .map_or(5, |value| value.parse().unwrap()),
            ),
            redis_connection_timeout: Duration::from_secs(
                var("REDIS_CONNECTION_TIMEOUT_SECS")
                    .ok()
                    .map_or(2, |value| value.parse().unwrap()),
            ),
            redis_max_delay: var("REDIS_MAX_DELAY_SECS")
                .ok()
                .map(|value| Duration::from_secs(value.parse().unwrap())),
            redis_connection_retry_count: var("REDIS_CONNECTION_RETRY_COUNT")
                .ok()
                .map_or(6, |value| value.parse().unwrap()),
            redis_publish_retry_count: var("REDIS_PUBLISH_RETRY_COUNT")
                .ok()
                .map_or(1, |value| value.parse().unwrap()),
        }
    }
}
