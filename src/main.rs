#![deny(clippy::complexity)]
#![deny(clippy::correctness)]
#![deny(clippy::nursery)]
#![deny(clippy::pedantic)]
#![deny(clippy::perf)]
#![deny(clippy::style)]
#![deny(clippy::suspicious)]

mod config;
mod ejson;
mod event;
mod metrics;
mod mongo;
mod redis;

use crate::{config::Config, mongo::Mongo, redis::Redis};
use metrics::{serve, LAST_EVENT_GAUGE, MONGO_COUNTER, REDIS_COUNTER};
use tokio::{main, spawn, sync::mpsc::channel};

#[main]
async fn main() {
    let mut config = Config::from_env();
    let mut mongo = Mongo::new(&config).await.unwrap();
    let mut redis = Redis::new(&config).await.unwrap();
    let (sender, mut receiver) = channel(1024);

    if let Some(metrics_address) = config.metrics_address.take() {
        spawn(serve(metrics_address));
    }

    spawn(async move {
        while let Some(event) = mongo.next().await.unwrap() {
            LAST_EVENT_GAUGE.set(event.ct.time.into());
            MONGO_COUNTER.inc();
            sender.send(event).await.unwrap();
        }
    });

    while let Some(event) = receiver.recv().await {
        REDIS_COUNTER.inc();
        redis.publish(&config, event).await.unwrap();
    }
}
