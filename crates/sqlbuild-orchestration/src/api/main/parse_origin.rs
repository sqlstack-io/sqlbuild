//! Browser authentication and CLI handoffs share one strict HTTPS origin contract.

use crate::api::_helpers::authentication::origins::parse_origin_with;
use crate::store::errors::ControlError;

pub fn parse_origin(value: &str, mechanism: &str) -> Result<String, ControlError> {
    parse_origin_with(value, mechanism, false)
}
