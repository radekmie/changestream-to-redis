use crate::{ejson::Ejson, event::Event, Config};
use redis::{aio::MultiplexedConnection, Client, RedisError, Script};

const SCRIPT_WITH_DEDUPLICATION: &str = r#"
    if redis.call("GET", KEYS[1]) == false then
        redis.call("SETEX", KEYS[1], ARGV[4], 1)
        redis.call("PUBLISH", ARGV[1], ARGV[3])
        redis.call("PUBLISH", ARGV[1] .. '::' .. ARGV[2], ARGV[3])
    end
"#;

const SCRIPT_WITHOUT_DEDUPLICATION: &str = r#"
    redis.call("PUBLISH", ARGV[1], ARGV[3])
    redis.call("PUBLISH", ARGV[1] .. '::' .. ARGV[2], ARGV[3])
"#;

pub struct Redis {
    connection: MultiplexedConnection,
    script: Script,
}

impl Redis {
    pub async fn new(config: &Config) -> Result<Self, RedisError> {
        let connection = Client::open(config.redis_url.as_str())?
            .get_multiplexed_async_connection()
            .await?;

        println!("Redis connection initialized.");
        let script = Script::new(match config.deduplication {
            None => SCRIPT_WITHOUT_DEDUPLICATION,
            Some(_) => SCRIPT_WITH_DEDUPLICATION,
        });

        Ok(Self { connection, script })
    }

    pub async fn publish(&mut self, config: &Config, event: Event) -> Result<(), RedisError> {
        let Event { ev, ns, id, op, .. } = event;
        if config.debug {
            println!("{}::{} {}", ns, id, op.clone().into_ejson());
        }

        let mut invocation = self.script.prepare_invoke();
        invocation.arg(ns);
        invocation.arg(id);
        invocation.arg(op.into_ejson().to_string());

        if let Some(deduplication) = config.deduplication {
            invocation.arg(deduplication);
            invocation.key(ev.to_string());
        }

        invocation.invoke_async(&mut self.connection).await
    }
}
