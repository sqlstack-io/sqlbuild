//! Every registered record spec, in registration order.

use crate::records::_helpers::native_classes::fields;
use crate::records::models::RecordSpec;

/// Every registered record spec, in registration order.
#[inline]
pub fn all_specs() -> impl Iterator<Item = &'static RecordSpec> {
    fields::all_specs()
}
