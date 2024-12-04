use std::time::Duration;

use crate::{ejson::Ejson, event::Event, Config};
use redis::{aio::ConnectionManager, Client, RedisError, Script};

const SCRIPT_WITH_DEDUPLICATION: &str = r#"
    for index = 1, tonumber(ARGV[1]) do
        if redis.call("GET", KEYS[index]) == false then
            local offset = index * 4 - 3
            redis.call("SETEX", KEYS[index], ARGV[offset + 4], 1)
            redis.call("PUBLISH", ARGV[offset + 1], ARGV[offset + 3])
            redis.call("PUBLISH", ARGV[offset + 1] .. '::' .. ARGV[offset + 2], ARGV[offset + 3])
        end
    end
"#;

const SCRIPT_WITHOUT_DEDUPLICATION: &str = r#"
    for index = 1, tonumber(ARGV[1]) do
        local offset = index * 3 - 2
        redis.call("PUBLISH", ARGV[offset + 1], ARGV[offset + 3])
        redis.call("PUBLISH", ARGV[offset + 1] .. '::' .. ARGV[offset + 2], ARGV[offset + 3])
    end
"#;

pub struct Redis {
    connection: ConnectionManager,
    script: Script,
}

impl Redis {
    pub async fn new(config: &Config) -> Result<Self, RedisError> {
        let connection = Client::open(config.redis_url.as_str())?
            .get_tokio_connection_manager_with_backoff_and_timeouts(
                2,
                100,
                6,
                Duration::from_secs(config.redis_response_timeout),
                Duration::from_secs(config.redis_connection_timeout),
            )
            .await?;

        println!("Redis connection initialized.");
        let script = Script::new(match config.deduplication {
            None => SCRIPT_WITHOUT_DEDUPLICATION,
            Some(_) => SCRIPT_WITH_DEDUPLICATION,
        });

        Ok(Self { connection, script })
    }

    pub async fn publish(&mut self, config: &Config, events: &[Event]) -> Result<(), RedisError> {
        if config.debug {
            for Event { ns, id, op, .. } in events {
                println!("{}::{} {}", ns, id, op.to_ejson());
            }
        }

        let mut invocation = self.script.prepare_invoke();
        invocation.arg(events.len());

        for Event { ev, ns, id, op, .. } in events {
            invocation.arg(ns);
            invocation.arg(id);
            invocation.arg(op.to_ejson().to_string());

            if let Some(deduplication) = config.deduplication {
                invocation.arg(deduplication);
                invocation.key(ev.to_string());
            }
        }

        invocation.invoke_async(&mut self.connection).await
    }
}
