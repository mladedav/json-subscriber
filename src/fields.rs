use std::{collections::HashMap, sync::{atomic::AtomicUsize, Arc}};

#[derive(Debug, Default)]
pub(crate) struct JsonFieldsInner {
    pub(crate) fields: HashMap<&'static str, serde_json::Value>,
    pub(crate) version: Arc<AtomicUsize>,
}

impl JsonFieldsInner {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            fields: HashMap::with_capacity(capacity),
            version: Arc::new(AtomicUsize::new(0)),
        }
    }

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
    pub(crate) current_versions: Vec<Arc<AtomicUsize>>,
    pub(crate) serialized_versions: Vec<usize>,
    pub(crate) serialized: Arc<str>,
}
