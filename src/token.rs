use crate::{config::Config, event::Event, redis::Redis};
use bson::{deserialize_from_slice, serialize_to_vec};
use mongodb::change_stream::event::ResumeToken;
use redis::RedisError;
use serde::{Deserialize, Serialize};

#[derive(Default, Deserialize, Serialize)]
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
}

impl From<&[u8]> for Tokens {
    fn from(value: &[u8]) -> Self {
        deserialize_from_slice(value)
            .inspect_err(|error| eprintln!("Failed to decode resume token: {error}"))
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
        let Some(ref key) = config.redis_resume_token_key else {
            return Ok(Self::default());
        };

        let tokens = redis.get(key).await?.map_or_else(
            || {
                println!(
                    "No change stream resume token found. Starting from the current position."
                );
                Tokens::default()
            },
            |bytes| {
                println!("Resume token found... Resuming the change streams.");
                Tokens::from(bytes.as_slice())
            },
        );

        Ok(Self {
            key: key.clone(),
            enabled: true,
            tokens,
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

        let mut primary: Option<&ResumeToken> = None;
        let mut secondary: Option<&ResumeToken> = None;

        for event in events.iter().rev() {
            if event.is_primary {
                primary.get_or_insert(&event.event_id);
            } else {
                secondary.get_or_insert(&event.event_id);
            }

            if primary.is_some() && secondary.is_some() {
                break;
            }
        }

        let primary = primary.or_else(|| self.tokens.primary());
        let secondary = secondary.or_else(|| self.tokens.secondary());

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
