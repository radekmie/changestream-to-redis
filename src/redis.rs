#![allow(clippy::module_name_repetitions)]

use crate::{ejson::Ejson, event::Event, Config};
use redis::{
    aio::{ConnectionManager, ConnectionManagerConfig},
    Client, RedisError, Script, ScriptInvocation,
};

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

pub struct RedisScript(Script);
pub struct RedisConnection(ConnectionManager);

pub struct Redis {
    pub connection: RedisConnection,
    pub script: RedisScript,
}

impl Redis {
    pub async fn new(config: &Config) -> Result<Self, RedisError> {
        let mut connection_config = ConnectionManagerConfig::new()
            .set_response_timeout(config.redis_response_timeout)
            .set_connection_timeout(config.redis_connection_timeout)
            .set_number_of_retries(config.redis_connection_retry_count);

        if let Some(max_delay) = config.redis_max_delay {
            // The Redis crate internally converts this back into a Duration, so this should be fine
            #[allow(clippy::cast_possible_truncation)]
            let max_delay_ms = max_delay.as_millis() as u64;

            connection_config = connection_config.set_max_delay(max_delay_ms);
        }

        let connection = RedisConnection(
            Client::open(config.redis_url.as_str())?
                .get_connection_manager_with_config(connection_config)
                .await?,
        );

        println!("Redis connection initialized.");
        let script = RedisScript(Script::new(match config.deduplication {
            None => SCRIPT_WITHOUT_DEDUPLICATION,
            Some(_) => SCRIPT_WITH_DEDUPLICATION,
        }));

        Ok(Self { connection, script })
    }
}

impl RedisScript {
    pub fn new_invocation(&self, config: &Config, events: Vec<Event>) -> ScriptInvocation {
        if config.debug {
            for Event { ns, id, op, .. } in &events {
                println!("{ns}::{id} {}", op.clone().into_ejson());
            }
        }

        let mut invocation = self.0.prepare_invoke();
        invocation.arg(events.len());

        for Event { ev, ns, id, op, .. } in events {
            invocation.arg(ns);
            invocation.arg(id);
            invocation.arg(op.into_ejson().to_string());

            if let Some(deduplication) = config.deduplication {
                invocation.arg(deduplication);
                invocation.key(ev.to_string());
            }
        }

        invocation
    }
}

impl RedisConnection {
    pub async fn publish<'a>(
        &mut self,
        invocation: &ScriptInvocation<'a>,
    ) -> Result<(), RedisError> {
        invocation.invoke_async(&mut self.0).await
    }
}
