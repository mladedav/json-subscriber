pub enum Value<'a> {
    SerdeJson(serde_json::Value),
    Str(&'a str),
    Serialized(&'a String),
}

impl Value<'_> {
    pub fn into_json(self) -> serde_json::Value {
        match self {
            Value::SerdeJson(value) => value,
            Value::Str(str) => serde_json::Value::from(str),
            // TODO FIXME
            Value::Serialized(str) => serde_json::from_str(str).unwrap(),
        }
    }
}
