//! Public traits and type aliases of project assembly.

use std::collections::HashMap;

/// `(placeholder defaults, dialect)` a statement parses with.
pub type ParseOptions = (Option<HashMap<String, String>>, Option<String>);
