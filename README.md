# changestream-to-redis

![CI](https://github.com/radekmie/changestream-to-redis/actions/workflows/ci.yml/badge.svg)
![Publish](https://github.com/radekmie/changestream-to-redis/actions/workflows/publish.yml/badge.svg)

> **Warning**
> The project is currently in its alpha phase. There are no production loads using it yet nor any large-scale tests were conducted. I do **not** recommend going live with it yet.

This program listens to a [MongoDB Change Stream](https://www.mongodb.com/docs/manual/changeStreams/), and publishes changes to Redis. It is designed to work with the [`cultofcoders:redis-oplog`](https://github.com/cult-of-coders/redis-oplog) Meteor package and meant as an alternative to to [`oplogtoredis`](https://github.com/tulip/oplogtoredis) with support for [`protectAgainstRaceConditions: false`](https://github.com/cult-of-coders/redis-oplog/blob/master/docs/finetuning.md#configuration-at-collection-level).

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
        * If set, all events are deduplicated on Redis. That allows you to deploy multiple instances of `oplogtoredis` listening to the same MongoDB database and pushing to the same Redis database.
    * (optional) `FULL_DOCUMENT`.
        * If not set, only IDs will be sent to Redis, i.e., it will behave just like `oplogtoredis`.
        * If set, it has to be [one of the values accepted by MongoDB](https://www.mongodb.com/docs/manual/reference/method/db.collection.watch/) (`required`, `updateLookup`, or `whenAvailable`), and you can configure your collections to use [`protectAgainstRaceConditions: false`](https://github.com/cult-of-coders/redis-oplog/blob/master/docs/finetuning.md#configuration-at-collection-level).

## Limitations

* **No change stream resumption.** It is planned, but at the moment the program is entirely stateless.
* **No error handling.** As soon as the change stream or Redis communication fails, the program exits. It is planned, though `changestream-to-redis` is meant to restart as soon as it exits.
* **No monitoring.** There is no monitoring of any kind, but both a healt-checking endpoint and [Prometheus](https://prometheus.io) metrics are planned.

## Development

This is a standard Rust application: `cargo fmt` will format the code, `cargo clippy` will check it, and `cargo test` will run tests.
