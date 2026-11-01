//! Codes, placement folders and wording of rules results and literal hints.

pub(crate) const LONG_LITERAL_CODE: &str = "SQLCLEAN044";
pub(crate) const QUOTE: char = '\'';
pub(crate) const LISTED_PATHS: usize = 5;
pub(crate) const MIN_SHARED_FILES: usize = 2;
pub(crate) const MIN_SCOPED_FOLDER_DEPTH: usize = 2;
pub(crate) const DECLARATION_GROUP: &str = "_sqlbuild";
pub(crate) const LOCAL_CONSTANTS: &str = "_sqlbuild/_constants/";
pub(crate) const INHERITED_CONSTANTS: &str = "_sqlbuild/constants/";
pub(crate) const PROJECT_CONSTANTS: &str = "constants/";
pub(crate) const FIX_AVAILABLE_NOTE: &str =
    "Fix available: `sqb format --fix` applies it after compiler verification";
pub(crate) const JSON_INDENT: usize = 2;
pub(crate) const CURRENT_DIRECTORY: &str = ".";
pub(crate) const UNEVALUATED_CODE: &str = "rules-unevaluated";
