//! Constants of the added-column walk.

/// Node kinds the parser keeps as unparsed text, whose columns the walk cannot see.
pub(crate) const OPAQUE_KINDS: [&str; 2] = ["command", "raw"];
/// The aggregate whose `*` argument counts rows.
pub(crate) const COUNT_FUNCTION: &str = "COUNT";
/// Relations in one scope from which an unqualified name could bind to either of two.
pub(crate) const SHARED_SCOPE_RELATIONS: usize = 2;
