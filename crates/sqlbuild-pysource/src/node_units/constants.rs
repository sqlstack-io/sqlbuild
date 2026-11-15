//! The names a node unit module is read for.

/// The function a node or check delegates to.
pub(crate) const MAIN_FUNCTION: &str = "main";
/// The decorator keyword naming what a node or check depends on.
pub(crate) const DEPENDS_ON_KEYWORD: &str = "depends_on";
/// How a `None` member of a return annotation is named.
pub(crate) const NONE_NAME: &str = "None";
/// The typing form that adds `None` to its member.
pub(crate) const OPTIONAL_NAME: &str = "Optional";
/// The typing forms whose members a return annotation unions.
pub(crate) const UNION_NAMES: [&str; 2] = ["Union", OPTIONAL_NAME];
/// The typing flag that guards imports needed only for annotations.
pub(crate) const TYPE_CHECKING_NAME: &str = "TYPE_CHECKING";
