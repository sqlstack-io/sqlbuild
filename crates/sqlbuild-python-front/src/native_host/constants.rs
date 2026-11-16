//! Public constants of native host.

/// Rows per `INSERT`; SQL Server accepts at most 1,000 rows in one `VALUES` list.
pub const ROWS_PER_STATEMENT: usize = 500;
