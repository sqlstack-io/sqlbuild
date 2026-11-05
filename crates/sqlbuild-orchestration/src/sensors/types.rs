//! Compound sensor return values.

use crate::store::models::ControlTransaction;

/// Selected source definitions by node key, and the key the next page starts after.
pub(crate) type SourcePage = (
    ControlTransaction,
    (Vec<(String, serde_json::Value)>, Option<String>),
);
