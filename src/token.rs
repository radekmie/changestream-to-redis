use crate::{config::Config, event::Event, redis::Redis};
use bson::{deserialize_from_slice, serialize_to_vec};
use mongodb::change_stream::event::ResumeToken;
use redis::RedisError;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct Tokens {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    primary: Option<ResumeToken>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    secondary: Option<ResumeToken>,
}

impl Tokens {
    pub const fn primary(&self) -> Option<&ResumeToken> {
        self.primary.as_ref()
    }

    pub const fn secondary(&self) -> Option<&ResumeToken> {
        self.secondary.as_ref()
    }

    const fn new(primary: Option<ResumeToken>, secondary: Option<ResumeToken>) -> Self {
        Self { primary, secondary }
    }

    fn encode(&self) -> Vec<u8> {
        serialize_to_vec(self).expect("Resume token is not serializable")
    }

    fn decode(bytes: &[u8]) -> Self {
        deserialize_from_slice(bytes)
            .inspect_err(|e| eprintln!("Failed to decode resume token: {e}"))
            .unwrap_or_default()
    }
}

#[derive(Default)]
pub struct TokenStore {
    tokens: Tokens,
    enabled: bool,
    key: String,
}

impl TokenStore {
    pub async fn from_config(config: &Config, redis: &mut Redis) -> Result<Self, RedisError> {
        if !config.enable_session_resumption {
            return Ok(Self::default());
        }

        let key = config.resume_token_redis_key.clone();

        let tokens = redis.get(&key).await?.map_or_else(
            || {
                // If not found, simply start from the current position, which is what a store-less run does anyway
                println!(
                    "No change stream resume token found. Starting from the current position."
                );
                Tokens::default()
            },
            |bytes| {
                println!("Resume token found! Resuming the change streams.");
                Tokens::decode(&bytes)
            },
        );

        Ok(Self {
            enabled: config.enable_session_resumption,
            tokens,
            key,
        })
    }

    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub const fn get_tokens(&self) -> &Tokens {
        &self.tokens
    }

    pub fn update(&mut self, events: &[Event]) {
        if !self.is_enabled() {
            return;
        }

        let cloned_tokens = self.tokens.clone();

        let mut secondary = cloned_tokens.secondary();
        let mut primary = cloned_tokens.primary();

        for event in events {
            if event.is_primary {
                primary = Some(&event.event_id);
            } else {
                secondary = Some(&event.event_id);
            }
        }

        self.tokens = Tokens::new(primary.cloned(), secondary.cloned());
    }

    pub fn serialize(&self) -> Option<(String, Vec<u8>)> {
        if self.enabled {
            Some((self.key.clone(), self.tokens.encode()))
        } else {
            None
        }
    }
}
