use base64::engine::{general_purpose::STANDARD, Engine};
use bson::Bson;
use serde_json::{json, Value};

pub trait Ejson {
    fn to_ejson(&self) -> Value;
}

impl Ejson for Bson {
    fn to_ejson(&self) -> Value {
        match self {
            // Meteor EJSON serialization.
            Self::Binary(v) => json!({ "$binary": STANDARD.encode(&v.bytes)}),
            Self::DateTime(v) => json!({ "$date": v.timestamp_millis() }),
            Self::Decimal128(v) => json!({ "$type": "Decimal", "$value": v.to_string() }),
            Self::Double(v) if v.is_infinite() => json!({ "$InfNaN": v.signum() }),
            Self::Double(v) if v.is_nan() => json!({ "$InfNan": 0 }),
            Self::ObjectId(v) => json!({ "$type": "oid", "$value": v.to_hex() }),
            Self::RegularExpression(v) => json!({ "$regexp": v.pattern, "$flags": v.options }),

            // Standard JSON serialization.
            Self::Array(v) => Value::Array(v.iter().map(Ejson::to_ejson).collect()),
            Self::Boolean(v) => json!(v),
            Self::Document(v) => Value::Object(
                v.into_iter()
                    .map(|(k, v)| (k.clone(), v.to_ejson()))
                    .collect(),
            ),
            Self::Double(v) => json!(v),
            Self::Int32(v) => json!(v),
            Self::Int64(v) => json!(v),
            Self::Null => Value::Null,
            Self::String(v) => json!(v),

            // Replace everything else with `null`s.
            v => {
                println!("Unrecognized BSON value found: {v}");
                Value::Null
            }
        }
    }
}
