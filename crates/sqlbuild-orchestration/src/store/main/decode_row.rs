//! Decode driver-neutral typed records through the store's shared row contract.

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::store::errors::ControlError;
use crate::store::types::Row;

pub(crate) fn decode_row<T: DeserializeOwned>(row: Row) -> Result<T, ControlError> {
    serde_json::from_value(Value::Object(row.into_iter().collect()))
        .map_err(|error| ControlError::Invalid(error.to_string()))
}
