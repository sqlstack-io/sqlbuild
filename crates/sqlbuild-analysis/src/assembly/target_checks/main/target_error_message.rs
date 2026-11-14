//! The message Python raises for a target namespace check failure.

use sqlbuild_core::text::main::python_str_repr::python_str_repr;

use crate::assembly::target_checks::constants::PRESERVE_TARGET_VALUE;
use crate::assembly::target_checks::errors::TargetError;
use crate::assembly::target_checks::main::missing_target_parts::missing_target_parts;

/// Python's `repr` of an optional string.
fn optional_repr(value: Option<&String>) -> String {
    value.map_or_else(|| "None".to_owned(), |text| python_str_repr(text))
}

/// The `PlannerInputError` message of `error`.
pub fn target_error_message(error: &TargetError) -> String {
    match error {
        TargetError::MissingParts(parts) => format!(
            "{} execution requires explicit target {}. {} '{}' resolved to database={} schema={}. \
             Set them in the model config, the environment config, or the project defaults.",
            parts.adapter_name,
            missing_target_parts(parts.database.as_deref(), parts.schema.as_deref()).join(", "),
            parts.resource_kind,
            parts.resource_name,
            optional_repr(parts.database.as_ref()),
            optional_repr(parts.schema.as_ref()),
        ),
        TargetError::LoaderCollision(collision) => format!(
            "Managed loader write collision: targets '{left}' and '{right}' both resolve to \
             database={} schema={}. Set distinct targets.{left}.loader_schema and \
             targets.{right}.loader_schema values.",
            optional_repr(collision.database.as_ref()),
            optional_repr(collision.schema.as_ref()),
            left = collision.left_target,
            right = collision.right_target,
        ),
        TargetError::ManagedWriteSchema(label) => format!(
            "{label} has no explicitly resolved physical write schema. Set schema on the \
             resource or its defaults, the selected target or its loader_schema, or the \
             project's only top-level [connection]."
        ),
        TargetError::NamedTargetSchema(target) => format!(
            "Named target '{target}' must explicitly set schema to a nonblank literal or \
             '{PRESERVE_TARGET_VALUE}'. Resource schemas, defaults, and connection schemas do \
             not select a named target schema strategy."
        ),
        TargetError::PreservedLoaderSchema(loader) => format!(
            "Intermediate loader '{loader}' would write to a schema named \
             '{PRESERVE_TARGET_VALUE}'. The target's schema '{PRESERVE_TARGET_VALUE}' keeps each \
             model's own schema, and a loader has none, so its destination must name a schema."
        ),
    }
}
