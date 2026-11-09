//! Node error text is normalized while a run change is read, so no caller stores or returns it raw.

use serde::{Deserialize, Deserializer};

use crate::runs::main::node_error_text::node_error_text;

pub(crate) fn bounded_error_text<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.map(|text| node_error_text(&text)))
}
