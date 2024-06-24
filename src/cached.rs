use std::sync::Arc;

pub(crate) enum Cached {
    Raw(Arc<str>),
    RawString(Arc<String>),
    Array(Vec<Arc<String>>),
}
