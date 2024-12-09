# changestream-to-redis

![CI](https://github.com/radekmie/changestream-to-redis/actions/workflows/ci.yml/badge.svg)
![Publish](https://github.com/radekmie/changestream-to-redis/actions/workflows/publish.yml/badge.svg)

This program listens to a [MongoDB Change Stream](https://www.mongodb.com/docs/manual/changeStreams/), and publishes changes to Redis. It is designed to work with the [`cultofcoders:redis-oplog`](https://github.com/cult-of-coders/redis-oplog) Meteor package and meant as an alternative to [`oplogtoredis`](https://github.com/tulip/oplogtoredis) with support for [`protectAgainstRaceConditions: false`](https://github.com/cult-of-coders/redis-oplog/blob/master/docs/finetuning.md#configuration-at-collection-level).

## Setup

1. Install `redis-oplog` with the following configuration:
    * `externalRedisPublisher: true`.
    * `globalRedisPrefix: "${database}."`, e.g., `globalRedisPrefix: "meteor."`.
2. Deploy `changestream-to-redis` with the following environmental variables:
    * (required) `MONGO_URL`, e.g., `mongodb://localhost:27017/meteor`.
    * (required) `REDIS_URL`, e.g., `redis://localhost:6379/1`.
    * (optional) `DEBUG`.
        * If set, all events are logged before being sent to Redis.
    * (optional) `DEDUPLICATION`, e.g., `120`.
        * If set, all events are deduplicated on Redis for this amount of seconds. That allows you to deploy multiple instances of `changestream-to-redis` listening to the same MongoDB database and pushing to the same Redis database.
    * (optional) `EXCLUDED_COLLECTIONS`, e.g., `exports,logs`.
        * If set, events from these collections will be ignored (i.e., won't get published to Redis). It allows you reduce `changestream-to-redis` and Redis load by ignoring write-intensive collections that don't require reactivity.
    * (optional) `FULL_DOCUMENT`.
        * If not set, only IDs will be sent to Redis, i.e., it will behave just like `oplogtoredis`.
        * If set, it has to be [one of the values accepted by MongoDB](https://www.mongodb.com/docs/manual/reference/method/db.collection.watch/) (`required`, `updateLookup`, or `whenAvailable`), and you can configure your collections to use [`protectAgainstRaceConditions: false`](https://github.com/cult-of-coders/redis-oplog/blob/master/docs/finetuning.md#configuration-at-collection-level).
    * (optional) `FULL_DOCUMENT_COLLECTIONS`, e.g., `notifications,users`.
        * If not set, there will be one change stream, fetching full documents from all collections, according to the `FULL_DOCUMENT` flag.
        * If set, there will be two change streams. First, listening to the configured collections, fetching full documents when available (i.e., inserts) and according to the `FULL_DOCUMENT` flag. Second will listen to other collections, fetching only their IDs.
    * (optional) `METRICS_ADDRESS`, e.g., `0.0.0.0:4000`.
        * If set, `changestream-to-redis` will expose Prometheus metrics at this address.
    * (optional) `REDIS_BATCH_SIZE`, default `1`.
        * If set, it overrides the default Redis batch size, leading to an increased throughput at a cost of increased latency (larger batches result in fewer but larger requests sent to Redis).
    * (optional) `REDIS_QUEUE_SIZE`, default `1024`.
        * If set, it overrides the default Redis queue size, accepting the MongoDB events earlier and temporarily storing them in memory.
    * (optional) `REDIS_RESPONSE_TIMEOUT_SECS`, default `5`.
        * [See docs](https://docs.rs/redis/latest/redis/aio/struct.ConnectionManagerConfig.html#method.set_response_timeout)
    * (optional) `REDIS_CONNECTION_TIMEOUT_SECS`, default `2`.
        * [See docs](https://docs.rs/redis/latest/redis/aio/struct.ConnectionManagerConfig.html#method.set_connection_timeout)

## Limitations

* **No change stream resumption.** It is planned, but at the moment the program is entirely stateless.
* **No error handling.** As soon as the change stream or Redis communication fails, the program exits. It is planned, though `changestream-to-redis` is meant to restart as soon as it exits.

## Performance

As reported on [Meteor forums](https://forums.meteor.com/t/introduction-of-changestream-to-redis/60269/8?u=radekmie), after a couple of weeks in a pre-production environment and a week in a production one, the performance is significantly better than `oplogtoredis`'s:
* **4x CPU and 5x RAM reduction in a pre-production environment** with low traffic and occasional peaks (e.g., migrations).
* **2x CPU and 4x RAM reduction in a production environment** with high traffic and regular peaks (e.g., migrations and cron jobs).
* **60% cost reduction in a production environment** by using a smaller instance (x0.5) and ARM CPU (x0.8). The production environment uses the currently smallest AWS Fargate instance (0.25 CPU / 0.5GB RAM) and stays below 2% CPU (on average) and 0.8% RAM (at all times).
* Stabler CPU and RAM usage (the latter stays the same for hours and even days).

Remember: your mileage may vary!

## Development

This is a standard Rust application: `cargo fmt` will format the code, `cargo clippy` will check it, and `cargo test` will run tests.
