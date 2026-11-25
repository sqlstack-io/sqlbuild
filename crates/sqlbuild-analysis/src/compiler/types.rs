//! Type aliases of the compiler's public results.

/// A rendered comparison's SQL and each CTE body's start, end and origin in it.
pub type PlacedComparisonSql = (String, Vec<(usize, usize, String)>);
