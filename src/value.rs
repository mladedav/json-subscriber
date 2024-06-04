use std::borrow::Cow;

pub enum Value<'a> {
    Serde(Cow<'a, serde_json::Value>),
    Str(&'a str),
}
