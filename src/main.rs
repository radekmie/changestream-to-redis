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
use std::mem::replace;
use tokio::{main, spawn, sync::mpsc::channel};

#[main]
async fn main() {
    let mut config = Config::from_env();
    let mut mongo = Mongo::new(&config).await.unwrap();
    let mut redis = Redis::new(&config).await.unwrap();
    let (sender, mut receiver) = channel(config.redis_queue_size);

    if let Some(metrics_address) = config.metrics_address.take() {
        spawn(serve(metrics_address));
    }

    spawn(async move {
        while let Some(event) = mongo.next().await.unwrap() {
            LAST_EVENT_GAUGE.set(event.timestamp.time.into());
            MONGO_COUNTER.inc();
            sender.send(event).await.unwrap();
        }
    });

    let batch_size = config.redis_batch_size;
    let mut batch = Vec::with_capacity(batch_size);
    while receiver.recv_many(&mut batch, batch_size).await != 0 {
        REDIS_COUNTER.inc_by(batch.len() as u64);
        redis
            .publish(&config, replace(&mut batch, Vec::with_capacity(batch_size)))
            .await
            .unwrap();
    }
}
