//! Project-tree facts as custom rules read them, with the errors Python spells for each failure.

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use sqlbuild_core::json::models::JsonValue;
use sqlbuild_core::text::main::python_str_repr::python_str_repr;
use sqlbuild_rules::project_tree::errors::{
    ProjectTextError, ProjectTreeFileError, RelativePartsError, Utf8Failure, Utf8Reason,
};
use sqlbuild_rules::project_tree::main::project_path_escapes::project_path_escapes;
use sqlbuild_rules::project_tree::main::project_relative_parts::project_relative_parts;
use sqlbuild_rules::project_tree::main::project_tree_children::project_tree_children;
use sqlbuild_rules::project_tree::main::project_tree_descendants::project_tree_descendants;
use sqlbuild_rules::project_tree::main::project_tree_entries::project_tree_entries;
use sqlbuild_rules::project_tree::main::project_tree_glob::project_tree_glob;
use sqlbuild_rules::project_tree::main::pure_path_text::pure_path_text;
use sqlbuild_rules::project_tree::main::read_project_text::read_project_text;
use sqlbuild_rules::project_tree::main::resolve_project_path::resolve_project_path;
use sqlbuild_rules::project_tree::main::tracked_text_suffixes::tracked_text_suffixes;
use sqlbuild_rules::project_tree::models::ProjectTreeEntry;

use crate::rules::_helpers::fact::fact_encoder::Encoded;
use crate::rules::_helpers::fact::fact_encoder::FactEncoder;
use crate::rules::_helpers::fact::fact_encoder::FactFailure;

/// The qualified name of the `ProjectPath` record class.
const PROJECT_PATH_RECORD: &str = "sqlbuild._native.ProjectPath";
pub(crate) const RULE_USAGE_ERROR: &str = "RuleUsageError";
pub(crate) const VALUE_ERROR: &str = "ValueError";
const ESCAPES: &str = "project path escapes the project: ";
const MISSING: &str = "project file does not exist: ";
const SYMLINK: &str = "project file must not be a symlink: ";
const UNREADABLE: &str = "could not read project file ";
/// The error `os.lstat` raises for a path holding a NUL byte.
const EMBEDDED_NUL_MESSAGE: &str = "lstat: embedded null character in path";
/// `OSError` subclasses Python picks by `errno`, by the kind of error it names.
const ERRNO_CLASSES: [(ErrorKind, &str); 10] = [
    (ErrorKind::WouldBlock, "BlockingIOError"),
    (ErrorKind::BrokenPipe, "BrokenPipeError"),
    (ErrorKind::ConnectionAborted, "ConnectionAbortedError"),
    (ErrorKind::ConnectionRefused, "ConnectionRefusedError"),
    (ErrorKind::ConnectionReset, "ConnectionResetError"),
    (ErrorKind::AlreadyExists, "FileExistsError"),
    (ErrorKind::NotFound, "FileNotFoundError"),
    (ErrorKind::Interrupted, "InterruptedError"),
    (ErrorKind::PermissionDenied, "PermissionError"),
    (ErrorKind::TimedOut, "TimeoutError"),
];
const OS_ERROR: &str = "OSError";

/// An exception a fact raised: its class name and `str()`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Raised {
    pub(crate) kind: String,
    pub(crate) message: String,
}

impl Raised {
    pub(crate) fn new(kind: &str, message: String) -> Self {
        Self {
            kind: kind.to_owned(),
            message,
        }
    }

    fn usage(message: String) -> Self {
        Self::new(RULE_USAGE_ERROR, message)
    }
}

/// A recomputed fact: its encoded value, or the exception it raised.
pub(crate) type Outcome = Result<JsonValue, Raised>;

/// The project's sorted tree nodes, as `project_tree_paths` lists them.
pub(crate) fn tree_nodes(root: &Path) -> Vec<ProjectTreeEntry> {
    let mut entries: Vec<ProjectTreeEntry> = project_tree_entries(root);
    entries.sort_by(|left, right| {
        left.relative
            .as_os_str()
            .as_encoded_bytes()
            .cmp(right.relative.as_os_str().as_encoded_bytes())
    });
    entries
}

/// `ProjectPath` nodes encoded as the host encodes the records.
pub(crate) fn nodes_value(
    encoder: &FactEncoder,
    nodes: &[&ProjectTreeEntry],
) -> Encoded<JsonValue> {
    let mut values: Vec<JsonValue> = Vec::with_capacity(nodes.len());
    for node in nodes {
        values.push(encoder.dataclass(
            PROJECT_PATH_RECORD,
            vec![
                path_string(&node.relative)?,
                JsonValue::Bool(node.is_file),
                JsonValue::Bool(!node.is_file),
            ],
        )?);
    }
    Ok(JsonValue::Array(values))
}

/// The tree nodes a text fact selects, or the error it raised.
pub(crate) fn selected_nodes<'a>(
    nodes: &'a [ProjectTreeEntry],
    fact: TreeQuery<'_>,
) -> Result<Vec<&'a ProjectTreeEntry>, Raised> {
    let values: Vec<&Path> = nodes.iter().map(|node| node.relative.as_path()).collect();
    let indexes: Vec<usize> = match fact {
        TreeQuery::Children(path) => project_tree_children(&values, Path::new(path)),
        TreeQuery::Descendants(path) => {
            project_tree_descendants(&values, (!path.is_empty()).then(|| Path::new(path)))
        }
        TreeQuery::Glob(pattern) => {
            if project_path_escapes(Path::new(pattern)) {
                return Err(Raised::usage(format!("{ESCAPES}{pattern}")));
            }
            project_tree_glob(&values, Path::new(pattern))
        }
    };
    Ok(indexes.into_iter().map(|index| &nodes[index]).collect())
}

/// One node query over the tree.
#[derive(Clone, Copy)]
pub(crate) enum TreeQuery<'a> {
    Children(&'a str),
    Descendants(&'a str),
    Glob(&'a str),
}

/// `ProjectTree.relative_parts(path=path, under=under)` under the resolved `root`.
pub(crate) fn relative_parts(root: &Path, path: &str, under: &str) -> Result<Vec<PathBuf>, Raised> {
    let relative: PathBuf = PathBuf::from(pure_path_text(Path::new(path)));
    let under_path: Option<PathBuf> =
        (!under.is_empty()).then(|| PathBuf::from(pure_path_text(Path::new(under))));
    match project_relative_parts(&relative, root, under_path.as_deref(), resolve_project_path) {
        Ok(parts) => Ok(parts.into_iter().map(PathBuf::from).collect()),
        Err(RelativePartsError::Resolve(_) | RelativePartsError::Escapes) => {
            Err(Raised::usage(format!("{ESCAPES}{path}")))
        }
        Err(RelativePartsError::NotBeneath) => Err(Raised::usage(format!(
            "project path {path} is not beneath {under}"
        ))),
    }
}

/// `ProjectTree.read_text(path)` under the resolved `root`.
pub(crate) fn read_text(root: &Path, path: &str) -> Result<String, Raised> {
    read_project_text(root, Path::new(path)).map_err(|error| text_error(&error, path))
}

/// The exception `ProjectTree.read_text(path)` raises for `error`.
pub(crate) fn text_error(error: &ProjectTextError, path: &str) -> Raised {
    match error {
        ProjectTextError::File(error) => file_error(error, path),
        ProjectTextError::Read { path: file, errno } => Raised::usage(format!(
            "{UNREADABLE}{path}: {}",
            os_error_text(*errno, &file.to_string_lossy())
        )),
        ProjectTextError::Decode(failure) => {
            Raised::usage(format!("{UNREADABLE}{path}: {}", decode_text(failure)))
        }
    }
}

/// The exception `project_tree_file_path(root, path)` raises for `error`.
pub(crate) fn file_error(error: &ProjectTreeFileError, path: &str) -> Raised {
    let prefix: String = match error {
        ProjectTreeFileError::Escapes => ESCAPES.to_owned(),
        ProjectTreeFileError::EmbeddedNul => {
            return Raised::new(VALUE_ERROR, EMBEDDED_NUL_MESSAGE.to_owned());
        }
        ProjectTreeFileError::Missing => MISSING.to_owned(),
        ProjectTreeFileError::Untracked => format!(
            "compiler rules input must be a tracked file type ({}): ",
            tracked_text_suffixes().join(", ")
        ),
        ProjectTreeFileError::Symlink => SYMLINK.to_owned(),
        ProjectTreeFileError::Unreadable { path: file, errno } => {
            return Raised::new(
                os_error_class(*errno),
                os_error_text(*errno, &file.to_string_lossy()),
            );
        }
    };
    Raised::usage(format!("{prefix}{path}"))
}

/// The `OSError` subclass Python raises for `errno`.
pub(crate) fn os_error_class(errno: i32) -> &'static str {
    let kind: ErrorKind = std::io::Error::from_raw_os_error(errno).kind();
    ERRNO_CLASSES
        .iter()
        .find(|(code, _)| *code == kind)
        .map_or(OS_ERROR, |(_, class)| class)
}

/// `os.strerror(errno)`.
pub(crate) fn strerror(errno: i32) -> String {
    let text: String = std::io::Error::from_raw_os_error(errno).to_string();
    text.strip_suffix(&format!(" (os error {errno})"))
        .unwrap_or(&text)
        .to_owned()
}

/// `str(OSError(errno, os.strerror(errno), file))`.
fn os_error_text(errno: i32, file: &str) -> String {
    format!(
        "[Errno {errno}] {}: {}",
        strerror(errno),
        python_str_repr(file)
    )
}

/// `str(UnicodeDecodeError)` of Python's strict UTF-8 decoder.
fn decode_text(failure: &Utf8Failure) -> String {
    let reason: &str = match failure.reason {
        Utf8Reason::InvalidStart => "invalid start byte",
        Utf8Reason::InvalidContinuation => "invalid continuation byte",
        Utf8Reason::UnexpectedEnd => "unexpected end of data",
    };
    if failure.end - failure.start == 1 {
        return format!(
            "'utf-8' codec can't decode byte 0x{:02x} in position {}: {reason}",
            failure.byte, failure.start
        );
    }
    format!(
        "'utf-8' codec can't decode bytes in position {}-{}: {reason}",
        failure.start,
        failure.end - 1
    )
}

/// A path as a JSON string; undecodable paths hold surrogates `orjson` refuses.
pub(crate) fn path_string(path: &Path) -> Encoded<JsonValue> {
    match path.to_str() {
        Some(text) => Ok(JsonValue::String(text.to_owned())),
        None => Err(FactFailure::Unreproducible),
    }
}
