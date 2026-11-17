//! Shapes of relation marker calls.

use std::ops::Range;

/// A marker call's lower-case function, such as `__ref`, and the name it references.
pub type MarkerCall = (String, String);

/// A marker call's byte range in the source, and the byte range of the padded name that stands for it.
pub type MarkerRanges = (Range<usize>, Range<usize>);
