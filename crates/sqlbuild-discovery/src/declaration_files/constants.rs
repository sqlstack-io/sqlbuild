//! Directories and names Python module discovery lists.

/// The directory whose `*.py` files define warehouse Python functions.
pub const PYTHON_FUNCTIONS_ROOT: &str = "sql/functions/python";
/// The prefix of a path part that keeps a module out of public module discovery.
pub const PRIVATE_PREFIX: char = '_';
