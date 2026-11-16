//! The classes, error factories and project declarations the model loop reads.

use crate::input_loops::_helpers::resources::loop_classes::dict;
use crate::input_loops::_helpers::resources::loop_classes::hook_signature;
use crate::input_loops::_helpers::resources::loop_classes::input_view_classes;
use crate::input_loops::_helpers::resources::loop_classes::named_records;
use crate::input_loops::_helpers::resources::loop_classes::native_config_classes;
use crate::input_loops::_helpers::resources::loop_classes::schema_enums;
use crate::input_loops::_helpers::resources::loop_classes::visibility_classes;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::PyDict;
use pyo3::{IntoPyObjectExt, wrap_pyfunction};
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::by_attribute::by_attribute;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_project::compiled_views::main::project::declaration_rows_function::declaration_rows_function;

/// The classes, error factories and project declarations the model loop reads.
pub fn model_loop_classes<'py>(
    audit_factories: &Bound<'py, PyAny>,
    context: &Bound<'py, PyAny>,
    external_resolver: &Bound<'py, PyAny>,
    host_inputs: &Bound<'py, PyDict>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = context.py();
    let host = |key: &str| -> PyResult<Bound<'py, PyAny>> { host_inputs.as_any().get_item(key) };
    let resolver: Bound<'py, PyAny> = native_field(context, "declaration_resolver")?;
    let mut provider_names: Vec<String> = host("provider_names")?
        .try_iter()?
        .map(|name| name?.extract())
        .collect::<PyResult<_>>()?;
    provider_names.sort();
    let hooks: Bound<'py, PyDict> = dict(
        py,
        vec![
            (
                "functions",
                by_attribute(&host("hook_functions")?, "name")?.into_any(),
            ),
            ("provider_names", provider_names.into_bound_py_any(py)?),
            (
                "python_hook_entry",
                record_class(py, RecordId::PythonHookEntry)?,
            ),
            (
                "signature",
                wrap_pyfunction!(hook_signature, py)?.into_any(),
            ),
        ],
    )?;
    let record: Bound<'py, PyDict> = dict(
        py,
        vec![(
            "declaration_rows",
            declaration_rows_function(py)?.into_any(),
        )],
    )?;
    dict(
        py,
        vec![
            ("usage_record", record_class(py, RecordId::UsageRecord)?),
            ("compile_error", error_class(py, ErrorId::CompileInput)?),
            ("syntax_help", host("syntax_help")?),
            ("source_syntax_help", host("source_syntax_help")?),
            ("macros", host("macros")?),
            ("sql_hooks", host("sql_hooks")?),
            (
                "scope_path_error",
                error_class(py, ErrorId::InvalidScopePath)?,
            ),
            ("sql_hook_entry", record_class(py, RecordId::SqlHookEntry)?),
            (
                "audit_factories",
                by_attribute(audit_factories, "name")?.into_any(),
            ),
            (
                "generated_usage",
                enum_member(py, RecordId::UsageKind, "GENERATED")?,
            ),
            (
                "contract_enforced",
                enum_member(py, RecordId::ContractPolicy, "ENFORCED")?,
            ),
            (
                "model_schemas",
                native_field(context, "public_model_schemas")?,
            ),
            ("named_records", named_records(&resolver)?.into_any()),
            ("schema_enums", schema_enums(&resolver)?.into_any()),
            (
                "compiler_diagnostic",
                record_class(py, RecordId::CompilerDiagnostic)?,
            ),
            ("report_diagnostic", host("report_diagnostic")?),
            ("path_class", imported(py, "pathlib", "Path")?),
            ("external_resolver", external_resolver.clone()),
            (
                "reference_kind",
                record_class(py, RecordId::SqlReferenceKind)?,
            ),
            (
                "config",
                native_config_classes(&host("config_session")?, &host("record_template_reads")?)?
                    .into_any(),
            ),
            ("hooks", hooks.into_any()),
            (
                "visibility",
                visibility_classes(context, &host("visible_declarations")?)?.into_any(),
            ),
            ("record", record.into_any()),
            ("input_view", input_view_classes(py)?.into_any()),
        ],
    )
}
