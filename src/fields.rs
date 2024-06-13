use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Default)]
pub(crate) struct JsonFields {
    pub(crate) fields: BTreeMap<&'static str, serde_json::Value>,
    pub(crate) version: usize,
    pub(crate) serialized: Option<Arc<str>>,
}

impl serde::Serialize for JsonFields {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut serializer = serializer.serialize_map(Some(self.fields.len()))?;

        for (key, value) in &self.fields {
            serializer.serialize_entry(key, value)?;
        }

        serializer.end()
    }
}
