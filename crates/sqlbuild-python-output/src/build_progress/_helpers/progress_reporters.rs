//! Warehouse connection and planning phase progress reporters.

pub(crate) const CONNECTION_OPTIONS: (&[&str], &[&str]) = (
    &["adapter_name", "stream"],
    &[
        "blank_line_before_start",
        "blank_line_after_complete",
        "use_color",
    ],
);
pub(crate) const PROJECT_COMPILE_OPERATION: &str = "project_compile";
