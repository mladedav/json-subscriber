use std::{collections::HashMap, fmt, io, sync::Arc};

use arc_swap::ArcSwapOption;
use serde::{ser::SerializeMap, Serializer};
use tracing::field::{Field, FieldSet};

use crate::serde::JsonSubscriberFormatterInsideObject;

type FieldsInner = Arc<HashMap<&'static str, ArcSwapOption<String>>>;

#[derive(Debug, Default)]
pub(crate) struct JsonFields {
    name: &'static str,
    fields: FieldsInner,
}

impl JsonFields {
    pub(crate) fn new(fields: &FieldSet, name: &'static str) -> Self {
        let mut map = HashMap::with_capacity(fields.len() + 1);
        for field in fields {
            if field.name() == name {
                continue;
            }
            map.insert(Self::name(field.name()), ArcSwapOption::default());
        }
        Self {
            fields: Arc::new(map),
            name,
        }
    }

    pub(crate) fn fields(&self) -> &FieldsInner {
        &self.fields
    }

    pub(crate) fn set(&self, key: &Field, value: String) {
        if key.name() == "name" {
            return;
        }

        self.fields
            .get(Self::name(key.name()))
            .map(|entry| entry.store(Some(Arc::new(value))));
    }

    fn name(name: &'static str) -> &'static str {
        if name.starts_with("r#") {
            &name[2..]
        } else {
            name
        }
    }
}

pub(crate) struct AsObject {
    fields: Vec<FieldsInner>,
}

impl AsObject {
    pub(crate) fn new() -> Self {
        Self { fields: Vec::new() }
    }

    pub(crate) fn single(inner: FieldsInner) -> Self {
        Self {
            fields: vec![inner],
        }
    }

    pub(crate) fn push(&mut self, inner: FieldsInner) {
        self.fields.push(inner);
    }

    pub(crate) fn write<W: io::Write>(&self, writer: W) -> io::Result<()> {
        let mut serializer = serde_json::Serializer::with_formatter(
            writer,
            JsonSubscriberFormatterInsideObject::new(),
        );

        let mut serializer = serializer.serialize_map(None)?;
        for fields in &self.fields {
            for (key, value) in &**fields {
                if let Some(value) = &*value.load() {
                    serializer.serialize_entry(key, &**value)?;
                }
            }
        }
        serializer.end()?;
        Ok(())
    }
}
