use std::sync::Arc;

pub(crate) enum Cached {
    Raw(Arc<str>),
    Array(Vec<Arc<str>>),
}
