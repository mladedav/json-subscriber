use std::{collections::BTreeMap, sync::Arc};

#[derive(Debug, Default)]
pub(crate) struct JsonFieldsInner {
    pub(crate) fields: BTreeMap<&'static str, serde_json::Value>,
    pub(crate) version: usize,
}

impl JsonFieldsInner {
    pub(crate) fn finish(self) -> JsonFields {
        let serialized = serde_json::to_string(&self.fields).unwrap();
        let serialized = Arc::from(serialized.as_str());

        JsonFields {
            inner: self,
            serialized,
        }
    }
}

#[derive(Debug)]
pub(crate) struct JsonFields {
    pub(crate) inner: JsonFieldsInner,
    pub(crate) serialized: Arc<str>,
}

impl serde::Serialize for JsonFields {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let mut serializer = serializer.serialize_map(Some(self.inner.fields.len()))?;

        for (key, value) in &self.inner.fields {
            serializer.serialize_entry(key, value)?;
        }

        serializer.end()
    }
}

pub(crate) struct FlattenedSpanFields {
    pub(crate) versions: Vec<usize>,
    pub(crate) serialized: Arc<str>,
}
