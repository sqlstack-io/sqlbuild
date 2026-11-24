//! Whether a schema can see the schema it extends and its enum column types, and the S006s.

use crate::compile_usages::constants::{
    CURRENT_FOLDER, DECLARATION_STEM, INACCESSIBLE_CODE, SCHEMA_KIND,
};
use crate::compile_usages::models::{InaccessibleDiagnostic, NamedRecord, RejectedScopePath};
use crate::scope_index::main::path_visible::path_visible;

/// `declaration_lexical_path`: the declaration file path whose folder its owner controls.
pub(crate) fn lexical_path(record: &NamedRecord) -> String {
    format!("{}/{DECLARATION_STEM}", folder(record))
}

fn folder(record: &NamedRecord) -> &str {
    record
        .declared
        .owning_path
        .as_deref()
        .filter(|path| !path.is_empty())
        .unwrap_or(CURRENT_FOLDER)
}

/// The owner a message names: the owning folder, else the ownership root.
fn owner(record: &NamedRecord) -> &str {
    record
        .declared
        .owning_path
        .as_deref()
        .filter(|path| !path.is_empty())
        .unwrap_or(&record.ownership_root)
}

/// Whether `declaration` is visible from the schema `consumer`'s declaration file.
pub(crate) fn visible_from(
    declaration: &NamedRecord,
    consumer: &NamedRecord,
) -> Result<bool, RejectedScopePath> {
    let path: String = lexical_path(consumer);
    path_visible(&declaration.declared, &path).map_err(|_| RejectedScopePath {
        declared: declaration.declared.clone(),
        path,
    })
}

/// An S006 keyed by its path and the two names that identify it within the file.
fn inaccessible(
    path: &str,
    names: [&str; 2],
    message: String,
    help: String,
) -> InaccessibleDiagnostic {
    let [first, second] = names;
    InaccessibleDiagnostic {
        key: vec![
            INACCESSIBLE_CODE.to_owned(),
            path.to_owned(),
            first.to_owned(),
            second.to_owned(),
        ],
        message,
        path: path.to_owned(),
        help,
    }
}

/// The S006 of a schema extending a schema its folder cannot see.
pub(crate) fn hidden_parent(
    consumer: &NamedRecord,
    name: &str,
    parent: &NamedRecord,
) -> InaccessibleDiagnostic {
    inaccessible(
        &consumer.path,
        [SCHEMA_KIND, name],
        format!(
            "Schema '{name}' is not visible from '{}'. It is defined at {} with {} scope owned \
             by '{}'",
            consumer.path,
            parent.path,
            parent.declared.scope.as_str(),
            owner(parent)
        ),
        format!(
            "move '{}' to a schemas/ role owned by '{}' or one of its parent folders, or to the \
             project-wide sql/schemas/ when consumers span resource trees",
            parent.path,
            folder(consumer)
        ),
    )
}

/// The S006 of a schema column typed with an enum the schema cannot see.
pub(crate) fn hidden_enum(
    schema: (&NamedRecord, &str),
    column: (&str, &str),
    enum_record: &NamedRecord,
) -> InaccessibleDiagnostic {
    let (record, schema_name) = schema;
    let (column_name, enum_name) = column;
    inaccessible(
        &record.path,
        [schema_name, column_name],
        format!(
            "Schema '{schema_name}' in {} column '{column_name}' uses enum '{enum_name}', which \
             is not visible from the schema's location. The enum is defined at {} with {} scope \
             owned by '{}'",
            record.path,
            enum_record.path,
            enum_record.declared.scope.as_str(),
            owner(enum_record)
        ),
        "move the enum to a role visible from the schema's owning folder, or move the schema \
         next to the enum"
            .to_owned(),
    )
}
