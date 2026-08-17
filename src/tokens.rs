use bson::{deserialize_from_slice, serialize_to_vec};
use mongodb::change_stream::event::ResumeToken;
use serde::{Deserialize, Serialize};

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

    pub const fn new(primary: Option<ResumeToken>, secondary: Option<ResumeToken>) -> Self {
        Self { primary, secondary }
    }

    pub fn encode(&self) -> Vec<u8> {
        serialize_to_vec(self).expect("Resume token is not serializable")
    }
}

impl From<&[u8]> for ResumeTokens {
    fn from(value: &[u8]) -> Self {
        deserialize_from_slice(value)
            .inspect_err(|error| eprintln!("Failed to decode resume token: {error}"))
            .unwrap_or_default()
    }
}
