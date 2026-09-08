use bson::{deserialize_from_slice, serialize_to_vec};
use mongodb::change_stream::event::ResumeToken;
use serde::{Deserialize, Serialize};

use crate::event::Event;

#[derive(Clone, Default, Deserialize, Serialize)]
pub struct ResumeTokens {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    primary: Option<ResumeToken>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    secondary: Option<ResumeToken>,
}

impl ResumeTokens {
    pub const fn primary(&self) -> Option<&ResumeToken> {
        self.primary.as_ref()
    }

    pub const fn secondary(&self) -> Option<&ResumeToken> {
        self.secondary.as_ref()
    }

    pub fn from_events(events: &[Event], fallback: &Self) -> Self {
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

        let primary = primary.or(fallback.primary.as_ref());
        let secondary = secondary.or(fallback.secondary.as_ref());

        Self {
            primary: primary.cloned(),
            secondary: secondary.cloned(),
        }
    }
}

impl From<&[u8]> for ResumeTokens {
    fn from(value: &[u8]) -> Self {
        deserialize_from_slice(value)
            .inspect_err(|error| eprintln!("Failed to decode resume token: {error}"))
            .unwrap_or_default()
    }
}

impl From<Option<&[u8]>> for ResumeTokens {
    fn from(value: Option<&[u8]>) -> Self {
        value.map_or_else(
            || {
                println!(
                    "No change stream resume token found. Starting from the current position."
                );
                Self::default()
            },
            |bytes| {
                println!("Resume token found... Resuming the change streams.");
                Self::from(bytes)
            },
        )
    }
}

impl From<&ResumeTokens> for Vec<u8> {
    fn from(val: &ResumeTokens) -> Self {
        serialize_to_vec(val).expect("Resume token is not serializable")
    }
}
