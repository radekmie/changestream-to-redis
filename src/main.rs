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
mod mongo;
mod redis;

use crate::{config::Config, mongo::Mongo, redis::Redis};
use tokio::{main, spawn, sync::mpsc::channel};

#[main]
async fn main() {
    let config = Config::from_env();
    let mut mongo = Mongo::new(&config).await.unwrap();
    let mut redis = Redis::new(&config).await.unwrap();
    let (sender, mut receiver) = channel(1024);

    spawn(async move {
        while let Some(event) = mongo.next().await.unwrap() {
            sender.send(event).await.unwrap();
        }
    });

    while let Some(event) = receiver.recv().await {
        redis.publish(&config, event).await.unwrap();
    }
}
