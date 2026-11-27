//! Expand one authored SQL string's macros into a `MacroExpansionResult`.

use crate::render::main::calls::span_row::span_row;
use crate::render::main::expansion::macro_expansion_classes::macro_expansion_classes;
use crate::render::models::ExpansionRequest;
use pyo3::IntoPyObject;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::sync::PyOnceLock;
use pyo3::types::{PyDict, PyDictMethods, PyList, PyString, PyStringMethods, PyTuple};
use sqlbuild_host::entries::models::UserCodeHost;

use crate::discovery::main::failures::explicit_references_help::explicit_references_help;
use crate::render::_helpers::declarations::declaration_values::resolved_declaration_context;
use crate::render::_helpers::expansion::macro_expansion::expanded;
use crate::render::models::{ExpansionOutput, NativeMacroExpansion, StateInput};
use crate::render::types::SpanRow;
use crate::sqltext::main::extract_references::extract_references;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::RecordId;

const TYPED_REFERENCE_FUNCTIONS: [(&str, &str); 3] = [
    ("model", "__ref"),
    ("source", "__source"),
    ("seed", "__seed"),
];
/// The call prefix of every reference function, in `SqlReferenceKind` order.
const GENERATED_REFERENCE_MARKERS: [&str; 6] = [
    "__ref(",
    "__seed(",
    "__source(",
    "__dbt_ref(",
    "__udf(",
    "__table_fn(",
];
const BUILTINS_MODULE: &str = "builtins";
const MACRO_TOKEN: &str = "@";
const GENERATED_ALLOWED: &str = "macro-generated references";

pub(crate) static EXPANSION_CLASSES: PyOnceLock<Py<PyDict>> = PyOnceLock::new();

fn generated_classes(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let help: Bound<'_, PyAny> =
        PyString::new(py, &explicit_references_help(GENERATED_ALLOWED)).into_any();
    let markers: Bound<'_, PyAny> = PyList::new(py, GENERATED_REFERENCE_MARKERS)?.into_any();
    keywords(
        py,
        &[
            (
                "compiler_diagnostic",
                record_class(py, RecordId::CompilerDiagnostic)?,
            ),
            (
                "source_location",
                record_class(py, RecordId::SourceLocation)?,
            ),
            (
                "resource_identity",
                record_class(py, RecordId::ResourceIdentity)?,
            ),
            (
                "generic_syntax",
                native_field(
                    &new_record(py, RecordId::SqlLexicalSyntax, (), None)?,
                    "native_mapping",
                )?,
            ),
            ("generated_help", help),
            ("all_markers", markers),
        ],
    )
}

pub(crate) fn built_classes(py: Python<'_>) -> PyResult<Bound<'_, PyDict>> {
    let declaration_kind: Bound<'_, PyAny> = record_class(py, RecordId::DeclarationKind)?;
    let ref_kind: Bound<'_, PyAny> = host_entry(py, UserCodeHost::SqlResourceRefKind)?;
    let ref_kinds: Bound<'_, PyDict> = PyDict::new(py);
    let ref_functions: Bound<'_, PyDict> = PyDict::new(py);
    for (kind, function) in TYPED_REFERENCE_FUNCTIONS {
        ref_kinds.set_item(function, ref_kind.call1((kind,))?)?;
        ref_functions.set_item(kind, function)?;
    }
    keywords(
        py,
        &[
            ("compile_error", error_class(py, ErrorId::CompileInput)?),
            (
                "macro_argument_error",
                error_class(py, ErrorId::MacroArgument)?,
            ),
            (
                "mismatch_error",
                error_class(py, ErrorId::NativeStageMismatch)?,
            ),
            (
                "declaration_identity",
                record_class(py, RecordId::DeclarationIdentity)?,
            ),
            ("macro_kind", native_field(&declaration_kind, "MACRO")?),
            ("declaration_kind", declaration_kind),
            ("usage_record", record_class(py, RecordId::UsageRecord)?),
            (
                "runtime_usage",
                enum_member(py, RecordId::UsageKind, "RUNTIME")?,
            ),
            (
                "resource_identity",
                record_class(py, RecordId::ResourceIdentity)?,
            ),
            (
                "model_kind",
                enum_member(py, RecordId::ResourceKind, "MODEL")?,
            ),
            (
                "resource_ref",
                host_entry(py, UserCodeHost::SqlResourceRef)?,
            ),
            ("ref_kinds", ref_kinds.into_any()),
            ("ref_functions", ref_functions.into_any()),
            (
                "relation_renderer",
                host_entry(py, UserCodeHost::RelationRenderer)?,
            ),
            (
                "invocation_context",
                host_entry(py, UserCodeHost::MacroInvocationContext)?,
            ),
            ("generated", generated_classes(py)?.into_any()),
            ("int_type", imported(py, BUILTINS_MODULE, "int")?),
            ("float_type", imported(py, BUILTINS_MODULE, "float")?),
        ],
    )
}

/// `tuple(dict.fromkeys(items))`: the items without repeats, in first-seen order.
fn unique<'py>(py: Python<'py>, items: Vec<Py<PyAny>>) -> PyResult<Bound<'py, PyTuple>> {
    let seen: Bound<'py, PyDict> = PyDict::new(py);
    for item in items {
        seen.set_item(item, py.None())?;
    }
    PyTuple::new(py, seen.keys())
}

fn result_model<'py>(
    py: Python<'py>,
    pairs: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    new_record_with(py, RecordId::MacroExpansionResult, pairs)
}

/// The macro expansion of `request`'s SQL, or `None` when it calls no macro.
pub(crate) fn macro_output(request: ExpansionRequest<'_>) -> PyResult<Option<ExpansionOutput>> {
    let py: Python<'_> = request.sql.py();
    if !request.sql.to_str()?.contains(MACRO_TOKEN) {
        return Ok(None);
    }
    let declarations: Bound<'_, PyAny> = match (request.declarations, request.declaration_resolver)
    {
        (Some(declarations), _) => declarations,
        (None, Some(resolver)) => {
            resolved_declaration_context(&resolver, &request.file_path, None)?
        }
        (None, None) => py.None().into_bound(py),
    };
    let expansion: NativeMacroExpansion = NativeMacroExpansion::new(
        &request.file_path,
        StateInput {
            loaded_macros: request.loaded_macros.unbind(),
            overrides: request
                .macro_overrides
                .unwrap_or_else(|| PyDict::new(py))
                .unbind(),
            macro_context: request.macro_context.clone().unbind(),
            declarations: declarations.unbind(),
            consumer: request.consumer.map_or_else(|| py.None(), Bound::unbind),
        },
        native_field(&request.macro_context, "_enforce_explicit_references")?.is_truthy()?,
        macro_expansion_classes(py)?.unbind(),
    )?;
    let expansion: Bound<'_, NativeMacroExpansion> = Bound::new(py, expansion)?;
    let bridge: Bound<'_, PyAny> = host_entry(py, UserCodeHost::ActiveMacroBridge)?.call0()?;
    expanded(&expansion, request.sql.to_str()?, &bridge).map(Some)
}

pub(crate) fn expansion_result<'py>(request: ExpansionRequest<'py>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = request.sql.py();
    let sql: Bound<'py, PyString> = request.sql.clone();
    let Some(output) = macro_output(request)? else {
        return result_model(py, &[("sql", sql.into_any())]);
    };
    let mut spans: Vec<Bound<'py, PyAny>> = Vec::with_capacity(output.spans.len());
    for span in output.spans {
        let SpanRow {
            source_start,
            source_end,
            output_start,
            output_end,
        } = span_row(span);
        spans.push(new_record_with(
            py,
            RecordId::ExpansionSpan,
            &[
                ("source_start", source_start.into_pyobject(py)?.into_any()),
                ("source_end", source_end.into_pyobject(py)?.into_any()),
                ("output_start", output_start.into_pyobject(py)?.into_any()),
                ("output_end", output_end.into_pyobject(py)?.into_any()),
            ],
        )?);
    }
    let argument_references: Bound<'py, PyAny> = if output.argument_references.is_empty() {
        PyTuple::empty(py).into_any()
    } else {
        let calls: Vec<String> = output
            .argument_references
            .iter()
            .map(|(kind, name)| format!("{}(\"{name}\")", typed_reference_function(kind)))
            .collect();
        let syntax: Bound<'py, PyAny> = new_record(py, RecordId::SqlLexicalSyntax, (), None)?;
        extract_references(&calls.join(" "), &syntax)?.into_any()
    };
    result_model(
        py,
        &[
            ("sql", PyString::new(py, &output.sql).into_any()),
            ("spans", PyTuple::new(py, spans)?.into_any()),
            ("argument_references", argument_references),
            ("dependencies", unique(py, output.dependencies)?.into_any()),
            ("usages", unique(py, output.usages)?.into_any()),
        ],
    )
}

/// The typed reference function naming a `SqlResourceRefKind` value, such as `__ref`.
fn typed_reference_function(kind: &str) -> &'static str {
    TYPED_REFERENCE_FUNCTIONS
        .iter()
        .find(|(typed, _)| *typed == kind)
        .map_or("", |(_, function)| function)
}

fn optional<'py>(value: Bound<'py, PyAny>) -> Option<Bound<'py, PyAny>> {
    (!value.is_none()).then_some(value)
}

/// Expand authored Python macros in one SQL string, keeping substitution spans and facts.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn expand_sql_macros<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> = KeywordArguments::read(
        py,
        "expand_sql_macros",
        options,
        (
            &["sql", "file_path", "loaded_macros", "macro_context"],
            &[
                "macro_overrides",
                "declaration_resolver",
                "declarations",
                "consumer",
            ],
        ),
    )?;
    expansion_result(ExpansionRequest {
        sql: arguments.value("sql")?.cast_into()?,
        file_path: arguments.value("file_path")?,
        loaded_macros: arguments.value("loaded_macros")?.cast_into()?,
        macro_overrides: optional(arguments.value("macro_overrides")?)
            .map(Bound::cast_into)
            .transpose()?,
        macro_context: arguments.value("macro_context")?,
        declaration_resolver: optional(arguments.value("declaration_resolver")?),
        declarations: optional(arguments.value("declarations")?),
        consumer: optional(arguments.value("consumer")?),
    })
}
