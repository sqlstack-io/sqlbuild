//! Image sections as length-prefixed records over a table of each distinct string.

use sqlbuild_cache::store::errors::StoreDecodeError;

/// The most items a list reserves room for before reading them, against corrupt counts.
pub(crate) const MAX_RESERVED_ITEMS: usize = 1 << 16;

/// A stored value, or `None` when the section is damaged.
pub(crate) fn read<T>(value: Result<T, StoreDecodeError>) -> Option<T> {
    let Ok(value) = value else {
        return None;
    };
    Some(value)
}

/// A stored count or index, or `None` when it does not fit this platform.
pub(crate) fn length(value: u64) -> Option<usize> {
    let Ok(length) = usize::try_from(value) else {
        return None;
    };
    Some(length)
}
