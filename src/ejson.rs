use base64::engine::{general_purpose::STANDARD, Engine};
use bson::Bson;
use serde_json::{json, Value};

pub trait Ejson {
    fn into_ejson(self) -> Value;
}

impl Ejson for Bson {
    fn into_ejson(self) -> Value {
        match self {
            // Meteor EJSON serialization.
            Bson::Binary(v) => json!({ "$binary": STANDARD.encode(v.bytes)}),
            Bson::DateTime(v) => json!({ "$date": v.timestamp_millis() }),
            Bson::Decimal128(v) => json!({ "$type": "Decimal", "$value": v.to_string() }),
            Bson::Double(v) if v.is_infinite() => json!({ "$InfNaN": v.signum() }),
            Bson::Double(v) if v.is_nan() => json!({ "$InfNan": 0 }),
            Bson::ObjectId(v) => json!({ "$type": "oid", "$value": v.to_hex() }),
            Bson::RegularExpression(v) => json!({ "$regexp": v.pattern, "$flags": v.options }),

            // Standard JSON serialization.
            Bson::Array(v) => Value::Array(v.into_iter().map(Ejson::into_ejson).collect()),
            Bson::Boolean(v) => json!(v),
            Bson::Document(v) => {
                Value::Object(v.into_iter().map(|(k, v)| (k, v.into_ejson())).collect())
            }
            Bson::Double(v) => json!(v),
            Bson::Int32(v) => json!(v),
            Bson::Int64(v) => json!(v),
            Bson::Null => Value::Null,
            Bson::String(v) => json!(v),

            // Replace everything else with `null`s.
            v => {
                println!("Unrecognized BSON value found: {}", v);
                Value::Null
            }
        }
    }
}
