//! Names and separators of project macro module analysis.

/// Separator of dotted module names.
pub(crate) const MODULE_SEPARATOR: &str = ".";
/// The mark that starts a path suffix.
pub(crate) const SUFFIX_MARK: char = '.';
/// The alias name of a star import.
pub(crate) const STAR_IMPORT_NAME: &str = "*";
/// The prefix of private names.
pub(crate) const PRIVATE_PREFIX: &str = "_";
/// Characters that end the anchor part of an absolute path.
pub(crate) const PATH_ROOTS: [char; 2] = ['/', '\\'];
/// Separator of the names in a reported call or import cycle.
pub(crate) const CYCLE_SEPARATOR: &str = " -> ";
/// Prefix of the synthetic package macro modules are executed under.
pub(crate) const SYNTHETIC_PACKAGE_PREFIX: &str = "_sqlbuild_project_macros_";
/// Hex digits of the content digest naming the synthetic package.
pub(crate) const SYNTHETIC_DIGEST_LENGTH: usize = 16;
/// The module of the Jinja helper whose keyword arguments may pass macros to a template.
pub(crate) const JINJA_MODULE: &str = "sqlbuild.macros";
/// The Jinja helper's name in `JINJA_MODULE`.
pub(crate) const JINJA_FUNCTION: &str = "jinja";
