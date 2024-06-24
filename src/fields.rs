use std::{
    collections::HashMap,
    ops::Deref,
    sync::{atomic::AtomicUsize, Arc},
};

use arc_swap::ArcSwapOption;

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
        JsonFields {
            inner: self,
            serialized: ArcSwapOption::new(None),
        }
    }
}

#[derive(Debug)]
pub(crate) struct JsonFields {
    pub(crate) inner: JsonFieldsInner,
    serialized: ArcSwapOption<String>,
}

impl JsonFields {
    pub(crate) fn serialized(&self) -> Arc<String> {
        let maybe_serialized = self.serialized.load();
        if let Some(serialized) = &*maybe_serialized {
            serialized.clone()
        } else {
            let serialized = Arc::new(serde_json::to_string(&self.inner.fields).unwrap());

            self.serialized
                .compare_and_swap(&Option::<Arc<_>>::None, Some(serialized.clone()))
                .as_ref()
                .map(Arc::clone)
                .unwrap_or(serialized)
        }
    }
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
