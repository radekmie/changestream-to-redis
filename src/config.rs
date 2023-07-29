use mongodb::options::FullDocumentType;
use serde_json::from_str;
use std::env::var;

pub struct Config {
    /// If true, all events are logged before being sent to Redis.
    pub debug: bool,
    /// If present, all events are deduplicated on Redis. That allows you to deploy multiple
    /// instances of `oplogtoredis` listening to the same MongoDB database and pushing to the same
    /// Redis database.
    pub deduplication: Option<usize>,
    /// By default, only the `_id` field is present, matching the `oplogtoredis` behavior. However,
    /// thanks to the `fullDocument` option in change streams, we can get the entire document at
    /// the same time. Both `updateLookup` and `required` variants incur an additional performance
    /// cost, but it's most likely less than an additional query coming from the outside. Having
    /// the whole document in Redis allows us to use the `protectAgainstRaceConditions: false` in
    /// the app, skipping the database call entirely.
    pub full_document: Option<FullDocumentType>,
    pub mongo_url: String,
    pub redis_url: String,
}

impl Config {
    pub fn from_env() -> Self {
        Self {
            debug: var("DEBUG").is_ok(),
            deduplication: var("DEDUPLICATION")
                .ok()
                .map(|value| value.parse().unwrap()),
            full_document: var("FULL_DOCUMENT")
                .ok()
                .map(|value| from_str(format!("\"{value}\"").as_str()).unwrap()),
            redis_url: var("REDIS_URL").expect("REDIS_URL is required"),
            mongo_url: var("MONGO_URL").expect("MONGO_URL is required"),
        }
    }
}
