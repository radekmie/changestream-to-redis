use mongodb::options::FullDocumentType;
use redis::aio::ConnectionManagerConfig;
use serde_json::from_str;
use std::{env::var, time::Duration, vec::Vec};

macro_rules! var_parse {
    ($name:expr) => {
        var($name).ok().map(|value| value.parse().unwrap())
    };
}

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
    pub mongo_batch_size: Option<u32>,
    pub mongo_max_await_time: Option<Duration>,
    pub mongo_url: String,
    /// If set, `changestream-to-redis` will generate more Redis messages,
    /// imitating the `namespaces` option set in all operations of the defined
    /// collections.
    pub namespaces: Option<Vec<(String, String)>>,
    pub redis_batch_size: usize,
    #[expect(clippy::struct_field_names)]
    pub redis_connection_manager_config: ConnectionManagerConfig,
    pub redis_publish_retry_count: usize,
    pub redis_queue_size: usize,
    pub redis_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            debug: var("DEBUG").is_ok(),
            deduplication: var_parse!("DEDUPLICATION"),
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
            mongo_batch_size: var_parse!("MONGO_BATCH_SIZE"),
            mongo_max_await_time: var_parse!("MONGO_MAX_AWAIT_TIME_MILLIS")
                .map(Duration::from_millis),
            mongo_url: var("MONGO_URL").expect("MONGO_URL is required"),
            namespaces: var("NAMESPACES").ok().map(|value| {
                value
                    .split(',')
                    .map(|namespace| match namespace.split_once('.') {
                        None => panic!("Namespace has to include a dot (`.`)."),
                        Some(("", _)) => panic!("Namespace's collection name cannot be empty."),
                        Some((_, "")) => panic!("Namespace's field name cannot be empty."),
                        Some((collection, field)) => (collection.to_string(), field.to_string()),
                    })
                    .collect()
            }),
            redis_batch_size: var_parse!("REDIS_BATCH_SIZE").unwrap_or(1),
            redis_connection_manager_config: Self::redis_connection_manager_config_from_env(),
            redis_publish_retry_count: var_parse!("REDIS_PUBLISH_RETRY_COUNT").unwrap_or(0),
            redis_queue_size: var_parse!("REDIS_QUEUE_SIZE").unwrap_or(1024),
            redis_url: var("REDIS_URL").expect("REDIS_URL is required"),
        }
    }

    fn redis_connection_manager_config_from_env() -> ConnectionManagerConfig {
        let mut config = ConnectionManagerConfig::new()
            .set_connection_timeout(
                var_parse!("REDIS_CONNECTION_TIMEOUT_SECS").map(Duration::from_secs),
            )
            .set_response_timeout(
                var_parse!("REDIS_RESPONSE_TIMEOUT_SECS").map(Duration::from_secs),
            );

        if let Some(x) = var_parse!("REDIS_CONNECTION_RETRY_COUNT") {
            config = config.set_number_of_retries(x);
        }

        if let Some(x) = var_parse!("REDIS_MAX_DELAY_SECS").map(Duration::from_secs) {
            config = config.set_max_delay(x);
        }

        config
    }
}
