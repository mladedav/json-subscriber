pub enum Value<'a> {
    SerdeJson(serde_json::Value),
    Str(&'a str),
}

impl Value<'_> {
    pub fn to_json(self) -> serde_json::Value {
        match self {
            Value::SerdeJson(value) => value,
            Value::Str(str) => serde_json::Value::from(str),
        }
    }
}
