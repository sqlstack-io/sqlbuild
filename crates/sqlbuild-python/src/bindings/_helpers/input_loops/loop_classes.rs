//! The Python classes, project declarations and callbacks the native model loop builds with.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyFrozenSet, PyList, PyTuple};
use pyo3::{IntoPyObjectExt, pyfunction, wrap_pyfunction};
use sqlbuild_host::entries::models::FacadeHost;

use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::by_attribute::by_attribute;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

/// The materializations a model's storage policies are inherited for, besides none.
const TABLE_BACKED_MATERIALIZATIONS: [&str; 3] = ["TABLE", "INCREMENTAL", "SNAPSHOT"];
/// The declaration kinds whose project-owned records the loop names.
const NAMED_KINDS: [&str; 4] = ["AUDIT", "SCHEMA", "SQL_HOOK", "PYTHON_HOOK"];

fn dict<'py>(
    py: Python<'py>,
    entries: Vec<(&str, Bound<'py, PyAny>)>,
) -> PyResult<Bound<'py, PyDict>> {
    let built: Bound<'py, PyDict> = PyDict::new(py);
    for (key, value) in entries {
        built.set_item(key, value)?;
    }
    Ok(built)
}

/// The values of a table-backed `MaterializationType`, as text.
pub(crate) fn table_backed<'py>(py: Python<'py>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    TABLE_BACKED_MATERIALIZATIONS
        .iter()
        .map(|name| enum_member(py, RecordId::MaterializationType, name))
        .collect()
}

/// The builder, storage defaults and classes a model's `CompileModelConfig` is made of.
pub(crate) fn native_config_classes<'py>(
    session: &Bound<'py, PyAny>,
    record_template_reads: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = session.py();
    let table_backed: Vec<Bound<'py, PyAny>> = table_backed(py)?
        .iter()
        .map(|materialization| Ok(materialization.str()?.into_any()))
        .collect::<PyResult<_>>()?;
    dict(
        py,
        vec![
            ("builder", native_field(session, "builder")?),
            (
                "inherited_storage",
                native_field(session, "inherited_storage")?,
            ),
            ("table_backed", PyTuple::new(py, table_backed)?.into_any()),
            (
                "compile_model_config",
                record_class(py, RecordId::CompileModelConfig)?,
            ),
            (
                "resolved_retention",
                record_class(py, RecordId::ResolvedTimeTravelRetention)?,
            ),
            (
                "retention_source",
                enum_member(py, RecordId::TimeTravelRetentionSource, "MODEL")?,
            ),
            (
                "resolved_table_type",
                record_class(py, RecordId::ResolvedTableType)?,
            ),
            ("table_type", record_class(py, RecordId::TableType)?),
            (
                "table_type_source",
                enum_member(py, RecordId::TableTypeSource, "MODEL")?,
            ),
            ("record_template_reads", record_template_reads.clone()),
        ],
    )
}

/// Each project-owned generic audit's, schema's and hook's first record, by `(kind, name)`.
fn named_records<'py>(resolver: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = resolver.py();
    let records: Bound<'py, PyDict> = PyDict::new(py);
    if resolver.is_none() {
        return Ok(records);
    }
    let kinds: Vec<Bound<'py, PyAny>> = NAMED_KINDS
        .iter()
        .map(|name| enum_member(py, RecordId::DeclarationKind, name))
        .collect::<PyResult<_>>()?;
    let declarations: Bound<'py, PyAny> =
        native_field(resolver, "lookup")?.getattr("declarations")?;
    for pair in mapping_items(&declarations)?.try_iter()? {
        let (identity, records_of): (Bound<'py, PyAny>, Bound<'py, PyAny>) = pair?.extract()?;
        let kind: Bound<'py, PyAny> = native_field(&identity, "kind")?;
        if kinds.iter().any(|named| kind.is(named))
            && native_field(&identity, "owner")?.is_none()
            && records_of.is_truthy()?
        {
            records.set_item(
                (
                    native_field(&kind, "value")?,
                    native_field(&identity, "name")?,
                ),
                records_of.get_item(0)?,
            )?;
        }
    }
    Ok(records)
}

/// Project-owned enums by name, as `model_schema_enum_declarations` resolves column types.
fn schema_enums<'py>(resolver: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = resolver.py();
    let enums: Bound<'py, PyDict> = PyDict::new(py);
    if resolver.is_none() {
        return Ok(enums);
    }
    let enum_kind: Bound<'py, PyAny> = enum_member(py, RecordId::DeclarationKind, "ENUM")?;
    let enum_class: Bound<'py, PyAny> = record_class(py, RecordId::EnumDeclaration)?;
    let declarations: Bound<'py, PyAny> =
        native_field(resolver, "projection")?.getattr("declarations")?;
    for pair in mapping_items(&declarations)?.try_iter()? {
        let (identity, value): (Bound<'py, PyAny>, Bound<'py, PyAny>) = pair?.extract()?;
        if native_field(&identity, "kind")?.is(&enum_kind)
            && native_field(&identity, "owner")?.is_none()
            && value.is_instance(&enum_class)?
        {
            enums.set_item(native_field(&identity, "name")?, value)?;
        }
    }
    Ok(enums)
}

/// A hook function's parameters as `inspect.signature` reads them: name, kind, default.
#[pyfunction]
fn hook_signature<'py>(function: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyList>> {
    let py: Python<'py> = function.py();
    let inspect: Bound<'py, PyAny> = py.import("inspect")?.into_any();
    let empty: Bound<'py, PyAny> = native_field(&inspect, "Parameter")?.getattr("empty")?;
    let mut parameters: Vec<(String, i64, bool)> = Vec::new();
    for parameter in inspect
        .call_method1("signature", (function,))?
        .getattr("parameters")?
        .call_method0("values")?
        .try_iter()?
    {
        let parameter: Bound<'py, PyAny> = parameter?;
        parameters.push((
            native_field(&parameter, "name")?.extract()?,
            native_field(&parameter, "kind")?
                .call_method0("__int__")?
                .extract()?,
            !native_field(&parameter, "default")?.is(&empty),
        ));
    }
    PyList::new(py, parameters)
}

/// What the loop resolves each model's visible declarations from.
fn visibility_classes<'py>(
    context: &Bound<'py, PyAny>,
    visible_declarations: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = context.py();
    let resolver: Bound<'py, PyAny> = native_field(context, "declaration_resolver")?;
    let (projects, resource_specific) = if resolver.is_none() {
        (false, PyFrozenSet::empty(py)?)
    } else {
        let lookup: Bound<'py, PyAny> = native_field(&resolver, "lookup")?;
        let declarations: Bound<'py, PyAny> =
            native_field(&lookup, "index")?.getattr("declarations")?;
        let private: Bound<'py, PyAny> = enum_member(py, RecordId::ScopeKind, "PRIVATE")?;
        let mut owners: Vec<Bound<'py, PyAny>> = Vec::new();
        for declaration in declarations.try_iter()? {
            let declaration: Bound<'py, PyAny> = declaration?;
            let owner: Bound<'py, PyAny> =
                native_field(&declaration, "identity")?.getattr("owner")?;
            if native_field(&declaration, "scope")?.is(&private) && !owner.is_none() {
                owners.push(owner);
            }
        }
        for resource in native_field(&lookup, "grants_by_resource")?.try_iter()? {
            owners.push(resource?);
        }
        (declarations.is_truthy()?, PyFrozenSet::new(py, &owners)?)
    };
    dict(
        py,
        vec![
            ("resolver", resolver),
            ("projects", projects.into_bound_py_any(py)?),
            ("resource_specific", resource_specific.into_any()),
            ("public_enums", native_field(context, "public_enums")?),
            (
                "public_constants",
                native_field(context, "public_constants")?,
            ),
            ("visible_declarations", visible_declarations.clone()),
            (
                "resource_identity",
                record_class(py, RecordId::ResourceIdentity)?,
            ),
            (
                "declaration_identity",
                record_class(py, RecordId::DeclarationIdentity)?,
            ),
            (
                "model_kind",
                enum_member(py, RecordId::ResourceKind, "MODEL")?,
            ),
            (
                "enum_kind",
                enum_member(py, RecordId::DeclarationKind, "ENUM")?,
            ),
            (
                "constant_kind",
                enum_member(py, RecordId::DeclarationKind, "CONSTANT")?,
            ),
            (
                "private_owner",
                enum_member(py, RecordId::VisibilityReason, "PRIVATE_OWNER")?,
            ),
            (
                "visibility_record",
                record_class(py, RecordId::VisibilityRecord)?,
            ),
        ],
    )
}

fn input_view_classes<'py>(py: Python<'py>) -> PyResult<Bound<'py, PyDict>> {
    let compiled: Vec<(&str, Bound<'py, PyAny>)> = vec![
        ("object_key", record_class(py, RecordId::CompiledObjectKey)?),
        (
            "relation_location",
            record_class(py, RecordId::CompiledRelationLocation)?,
        ),
        (
            "inferred_column",
            record_class(py, RecordId::InferredColumn)?,
        ),
        (
            "nullability",
            record_class(py, RecordId::InferredNullability)?,
        ),
        (
            "compiler_diagnostic",
            record_class(py, RecordId::CompilerDiagnostic)?,
        ),
        (
            "source_location",
            record_class(py, RecordId::SourceLocation)?,
        ),
        (
            "compact_lineage_facts",
            record_class(py, RecordId::CompactLineageFacts)?,
        ),
        (
            "lineage_column_fact",
            record_class(py, RecordId::CompiledLineageColumnFact)?,
        ),
        (
            "lineage_source_fact",
            record_class(py, RecordId::CompiledLineageSourceFact)?,
        ),
        (
            "contract_proof",
            record_class(py, RecordId::DynamicColumnContractProof)?,
        ),
        (
            "family_proof",
            record_class(py, RecordId::DynamicColumnFamilyProof)?,
        ),
    ];
    dict(
        py,
        vec![
            (
                "sql_reference",
                record_class(py, RecordId::CompileSqlReference)?,
            ),
            (
                "reference_kind",
                record_class(py, RecordId::SqlReferenceKind)?,
            ),
            (
                "schema_model_entry",
                record_class(py, RecordId::SchemaModelEntry)?,
            ),
            (
                "sql_expansion",
                record_class(py, RecordId::CompiledSqlExpansion)?,
            ),
            ("expansion_span", record_class(py, RecordId::ExpansionSpan)?),
            (
                "audits",
                dict(
                    py,
                    vec![
                        ("audit_severity", host_entry(py, FacadeHost::AuditSeverity)?),
                        (
                            "attached_target_kind",
                            host_entry(py, FacadeHost::AttachedAuditTargetKind)?,
                        ),
                    ],
                )?
                .into_any(),
            ),
            (
                "functions",
                dict(
                    py,
                    vec![
                        (
                            "function_argument",
                            record_class(py, RecordId::FunctionArgument)?,
                        ),
                        (
                            "function_return_column",
                            record_class(py, RecordId::FunctionReturnColumn)?,
                        ),
                        (
                            "function_language",
                            host_entry(py, FacadeHost::FunctionLanguage)?,
                        ),
                    ],
                )?
                .into_any(),
            ),
            ("compiled", dict(py, compiled)?.into_any()),
        ],
    )
}

/// The classes, error factories and project declarations the model loop reads.
pub(crate) fn model_loop_classes<'py>(
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
            pyo3::wrap_pyfunction!(
                crate::bindings::_helpers::compiled_views::records::declaration_rows,
                py
            )?
            .into_any(),
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
