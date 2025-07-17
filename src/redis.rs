use crate::{ejson::Ejson, event::Event, Config};
use redis::{aio::ConnectionManager, Client, RedisError, Script};

const SCRIPT_WITH_DEDUPLICATION: &str = r#"
    for index = 1, tonumber(ARGV[1]) do
        if redis.call("GET", KEYS[index]) == false then
            local offset = index * 6 - 5
            redis.call("SETEX", KEYS[index], ARGV[offset + 6], 1)
            redis.call("PUBLISH", ARGV[offset + 1] .. '.' .. ARGV[offset + 2], ARGV[offset + 5])
            redis.call("PUBLISH", ARGV[offset + 1] .. '.' .. ARGV[offset + 2] .. '::' .. ARGV[offset + 4], ARGV[offset + 5])
            for namespace in ARGV[offset + 3]:gmatch('[^,]+') do
                redis.call("PUBLISH", ARGV[offset + 1] .. '.' .. namespace .. '::' .. ARGV[offset + 2], ARGV[offset + 5])
            end
        end
    end
"#;

const SCRIPT_WITHOUT_DEDUPLICATION: &str = r#"
    for index = 1, tonumber(ARGV[1]) do
        local offset = index * 5 - 4
        redis.call("PUBLISH", ARGV[offset + 1] .. '.' .. ARGV[offset + 2], ARGV[offset + 5])
        redis.call("PUBLISH", ARGV[offset + 1] .. '.' .. ARGV[offset + 2] .. '::' .. ARGV[offset + 4], ARGV[offset + 5])
        for namespace in ARGV[offset + 3]:gmatch('[^,]+') do
            redis.call("PUBLISH", ARGV[offset + 1] .. '.' .. namespace .. '::' .. ARGV[offset + 2], ARGV[offset + 5])
        end
    end
"#;

pub struct Redis {
    connection_manager: ConnectionManager,
    script: Script,
}

impl Redis {
    pub async fn new(config: &Config) -> Result<Self, RedisError> {
        let connection_manager = Client::open(config.redis_url.as_str())?
            .get_connection_manager_with_config(config.redis_connection_manager_config.clone())
            .await?;

        println!("Redis connection initialized.");
        let script = Script::new(match config.deduplication {
            None => SCRIPT_WITHOUT_DEDUPLICATION,
            Some(_) => SCRIPT_WITH_DEDUPLICATION,
        });

        Ok(Self {
            connection_manager,
            script,
        })
    }

    pub async fn publish(&mut self, config: &Config, events: Vec<Event>) -> Result<(), RedisError> {
        if config.debug {
            for event in &events {
                event.debug();
            }
        }

        let mut invocation = self.script.prepare_invoke();
        invocation.arg(events.len());

        for event in events {
            invocation.arg(event.db);
            invocation.arg(event.collection);
            invocation.arg(event.namespaces);
            invocation.arg(event.document_id);
            invocation.arg(event.operation.into_ejson().to_string());

            if let Some(deduplication) = config.deduplication {
                invocation.arg(deduplication);
                invocation.key(event.event_id.to_string());
            }
        }

        let retry_limit = config.redis_publish_retry_count;
        for retry in 0..=retry_limit {
            match invocation.invoke_async(&mut self.connection_manager).await {
                Ok(()) => {
                    if retry > 0 {
                        eprintln!("Redis publication succeeded (retry #{retry})");
                    }

                    return Ok(());
                }
                // All I/O errors can be safely retried.
                Err(error) if !error.is_io_error() || retry == retry_limit => return Err(error),
                Err(error) => {
                    eprintln!("Redis error (retry #{retry}): {error:?}");
                }
            }
        }

        unreachable!()
    }
}
