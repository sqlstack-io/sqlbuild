//! The value of the native string enum `enum_name`'s member `member`.

use crate::records::_helpers::native_classes::fields;

/// The value of the native string enum `enum_name`'s member `member`.
#[inline]
pub fn enum_member_value(enum_name: &str, member: &str) -> Option<&'static str> {
    fields::enum_member_value(enum_name, member)
}
