//! Whether `SQLBUILD_NO_PROGRESS` turns transient spinners off.

use crate::build_progress::_helpers::transient_status;

/// Whether `SQLBUILD_NO_PROGRESS` turns transient spinners off.
#[inline]
pub fn progress_spinners_disabled() -> bool {
    transient_status::progress_spinners_disabled()
}
