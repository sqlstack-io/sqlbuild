//! The compile-input model loop: each model's steps run in the attachment's order, natively.

use crate::input_loops::models::LoopSwitches;
use sqlbuild_python_base::records::main::construct::exception_from::exception_from;

use pyo3::FromPyObject;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_analysis::compiled_project::models::SplicedSpan;
use sqlbuild_analysis::sql_macros::main::moved_typed_calls::moved_typed_calls;
use sqlbuild_analysis::sql_macros::main::spliced_typed_calls::spliced_typed_calls;
use sqlbuild_analysis::sql_macros::models::{TextSplice, TypedCall};
use sqlbuild_attachments::cursor_intrinsics::main::replace_intrinsics_with_spans::replace_intrinsics_with_spans;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::input_loops::_helpers::model::model_loop_hooks::SqlHookClasses;
use crate::input_loops::_helpers::model::model_loop_host::DeclarationScan;
use crate::input_loops::_helpers::model::model_loop_host::InterpolationRow;
use crate::input_loops::_helpers::model::model_loop_host::ModelLoopRow;
use crate::input_loops::_helpers::model::model_loop_host::RecordClasses;
use crate::input_loops::_helpers::model_loop::model_loop_macros::MacroClasses;
use crate::input_loops::_helpers::model_loop::model_loop_python_hooks::HookClasses;
use crate::input_loops::_helpers::model_loop::model_loop_python_hooks::HookSignatures;
use crate::input_loops::_helpers::model_loop::model_loop_visibility::ModelVisibility;
use crate::input_loops::_helpers::model_loop::model_loop_visibility::VisibilityClasses;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::discovery::models::{ModelFileTexts, NativeModelFile};
use sqlbuild_python_front::model_config::main::config::raised_config_exception::raised_config_exception;
use sqlbuild_python_front::model_config::models::ConfigClasses;
use sqlbuild_python_front::model_config::models::ContractClasses;
use sqlbuild_python_front::render::models::ConstantRendering;
use sqlbuild_python_front::render::types::SpanRow;
use sqlbuild_python_front::sqltext::main::declaration_scan::declaration_scan;
use sqlbuild_python_front::sqltext::main::interpolation_batch::interpolation_batch;
use sqlbuild_python_front::sqltext::models::InterpolationRow as SqlInterpolationRow;
use sqlbuild_python_front::sqltext::models::InterpolationSources;

pub(crate) const MATERIALIZED_KEY: &str = "materialized";
pub(crate) const CURSOR_KEY: &str = "cursor";
pub(crate) const INCREMENTAL_MATERIALIZATION: &str = "incremental";
pub(crate) const VALUES_ATTRIBUTE: &str = "values";
pub(crate) const RETENTION_ATTRIBUTE: &str = "time_travel_retention";
pub(crate) const UNMANAGED_ATTRIBUTE: &str = "unmanaged";
pub(crate) const TABLE_TYPE_ATTRIBUTE: &str = "table_type";
pub(crate) const DECLARED_ATTRIBUTE: &str = "declared";
pub(crate) const VALIDATE_METHOD: &str = "validate";
pub(crate) const ANALYSIS_CONFLICT: &str =
    "MODEL sql_analysis conflicts with legacy sql_validation";
pub(crate) const ANALYSIS_CONFLICT_CODE: &str = "P003";
const SQL_HOOKS_KEY: &str = "sql_hooks";
const HOOK_FILES_KEY: &str = "hook_files";
const DEFINITIONS_KEY: &str = "definitions";
const COMPILE_ERROR_KEY: &str = "compile_error";

/// The native validator, header contract classes, and the loop's Python classes.
pub(crate) type LoopChecks = (Py<PyAny>, Py<PyDict>, Py<PyDict>);

/// `(analysis switches, project config path, the adapter's analysis dialect)`.
pub(crate) type LoopSettingsInput = (LoopSwitches, String, Option<String>);

/// The Python classes and error factory the loop builds usages, hooks and errors with.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct LoopClasses<'py> {
    pub(crate) usage_record: Bound<'py, PyAny>,
    pub(crate) compile_error: Bound<'py, PyAny>,
    pub(crate) sql_hook_entry: Bound<'py, PyAny>,
    /// Discovered audit factories by name.
    pub(crate) audit_factories: Bound<'py, PyDict>,
    /// `UsageKind.GENERATED`, for usages an enum-typed column generates.
    pub(crate) generated_usage: Bound<'py, PyAny>,
    /// `ContractPolicy.ENFORCED`.
    pub(crate) contract_enforced: Bound<'py, PyAny>,
    /// Public reusable model schemas by name.
    pub(crate) model_schemas: Bound<'py, PyDict>,
    /// Each indexed schema's and Python hook's declaration record, by `(kind, name)`.
    pub(crate) named_records: Bound<'py, PyDict>,
    /// Project enums by name, as a schema's column types resolve them.
    pub(crate) schema_enums: Bound<'py, PyDict>,
    /// `CompilerDiagnostic` and `report_compile_diagnostic`, for S006.
    pub(crate) compiler_diagnostic: Bound<'py, PyAny>,
    pub(crate) report_diagnostic: Bound<'py, PyAny>,
    /// `pathlib.Path`.
    pub(crate) path_class: Bound<'py, PyAny>,
    /// The dbt manifest's `ExternalSqlReferenceResolver`, or `None`.
    pub(crate) external_resolver: Bound<'py, PyAny>,
    /// `SqlReferenceKind`.
    pub(crate) reference_kind: Bound<'py, PyAny>,
    /// What a model's effective config is built from.
    pub(crate) config: ConfigClasses<'py>,
    /// What a model's visible declarations are resolved from.
    pub(crate) visibility: VisibilityClasses<'py>,
    /// What a model's Python hooks are checked against.
    pub(crate) hooks: HookClasses<'py>,
    /// What each model's SQL hooks are expanded with.
    pub(crate) sql_hooks: SqlHookClasses<'py>,
    /// What each model's macro calls are expanded with.
    pub(crate) macros: MacroClasses<'py>,
    /// `sql_analysis_opt_out_help(model_owned=True)`, the help of a model's syntax error.
    pub(crate) syntax_help: String,
    /// The same help for SQL a project's other resources own.
    pub(crate) source_syntax_help: String,
    /// `InvalidScopePathError`.
    pub(crate) scope_path_error: Bound<'py, PyAny>,
    /// What each finished model's input record is built with.
    pub(crate) record: RecordClasses<'py>,
    /// What model input views project their attributes with.
    pub(crate) input_view: Bound<'py, PyAny>,
}

/// What one run reads beside each model: the host, Python classes and header contract classes.
pub(crate) struct LoopContext<'a, 'py> {
    pub(crate) host: &'a Bound<'py, ConstantRendering>,
    pub(crate) classes: &'a LoopClasses<'py>,
    pub(crate) contracts: &'a ContractClasses<'py>,
    /// The lexical rules the project's SQL is authored in.
    pub(crate) syntax: &'a LexicalSyntax,
}

/// The declarations a model sees and the identity its usages are recorded for.
pub(crate) struct VisibleDeclarations<'py> {
    pub(crate) declarations: Bound<'py, PyAny>,
    pub(crate) consumer: Bound<'py, PyAny>,
}

/// What the model loop knows about a model before finishing it.
pub(crate) struct LoopModel<'a, 'py> {
    pub(crate) row: &'a ModelLoopRow,
    pub(crate) config: Bound<'py, PyAny>,
    pub(crate) visible: &'a VisibleDeclarations<'py>,
    pub(crate) signatures: &'a HookSignatures,
    pub(crate) visibility: &'a ModelVisibility<'py>,
}

/// Replace the SQL hook classes' discovered `hook_files` with their `definitions` by name.
pub(crate) fn index_sql_hook_definitions(loop_classes: &Bound<'_, PyDict>) -> PyResult<()> {
    let py: Python<'_> = loop_classes.py();
    let hooks: Bound<'_, PyDict> = loop_classes.as_any().get_item(SQL_HOOKS_KEY)?.cast_into()?;
    let definitions: Bound<'_, PyDict> = PyDict::new(py);
    for hook_file in hooks.as_any().get_item(HOOK_FILES_KEY)?.try_iter()? {
        let hook_file: Bound<'_, PyAny> = hook_file?;
        let name: Bound<'_, PyAny> = native_field(&hook_file, "name")?;
        if let Some(existing) = definitions.get_item(&name)? {
            let message: String = format!(
                "Duplicate SQL hook name '{}' found in {} and {}",
                name.str()?,
                native_field(&existing, "relative_path")?.str()?,
                native_field(&hook_file, "relative_path")?.str()?
            );
            let error: Bound<'_, PyAny> = loop_classes
                .as_any()
                .get_item(COMPILE_ERROR_KEY)?
                .call1((message,))?;
            return Err(PyErr::from_value(error));
        }
        definitions.set_item(name, hook_file)?;
    }
    hooks.set_item(DEFINITIONS_KEY, definitions)
}

/// Each model's `@@`-interpolated query and declaration scan, all interpolated before any runs.
pub(crate) fn prepared_rows<'py>(
    py: Python<'py>,
    (classes, syntax): (&LoopClasses<'py>, &LexicalSyntax),
    model_files: Vec<Py<NativeModelFile>>,
) -> PyResult<Vec<ModelLoopRow>> {
    let hooks = &classes.sql_hooks;
    let mut sqls: Vec<(String, String)> = Vec::with_capacity(model_files.len());
    let mut paths: Vec<ModelFileTexts> = Vec::with_capacity(model_files.len());
    for model_file in &model_files {
        let record: &NativeModelFile = model_file.get();
        let texts: ModelFileTexts = record.path_texts(py)?;
        sqls.push((record.query_text().to_string(), texts.file_path.clone()));
        paths.push(texts);
    }
    let interpolated: Vec<SqlInterpolationRow> = interpolation_batch(
        InterpolationSources(
            hooks.effective_vars.clone(),
            hooks.environment.clone(),
            None,
            hooks.render_project_var.clone(),
            Some(syntax.clone()),
        ),
        &sqls,
    )?;
    let mut rows: Vec<ModelLoopRow> = Vec::with_capacity(model_files.len());
    for (((model_file, paths), (authored, _)), interpolated) in model_files
        .into_iter()
        .zip(paths)
        .zip(sqls)
        .zip(interpolated)
    {
        let SqlInterpolationRow {
            sql: rendered,
            spans,
            reads,
            error,
        } = interpolated;
        let sql: String = rendered.unwrap_or(authored);
        let scan: Option<DeclarationScan> = error
            .is_none()
            .then(|| declaration_scan(&sql, Some(syntax)));
        let reads: Vec<(String, String)> = reads
            .into_iter()
            .map(|(kind, name)| (kind.to_owned(), name))
            .collect();
        let interpolation: InterpolationRow = InterpolationRow {
            sql,
            reads,
            error,
            spans,
        };
        rows.push(ModelLoopRow::new((model_file, paths), scan, interpolation));
    }
    Ok(rows)
}

/// `_raise_validation_outcome`: nothing, the validator's error, or a rejected reference's error.
pub(crate) fn validation_outcome(
    outcome: &Bound<'_, PyAny>,
    external: Vec<Option<PyErr>>,
    values: &Bound<'_, PyDict>,
) -> PyResult<()> {
    if outcome.is_none() {
        return Ok(());
    }
    if let Ok(index) = outcome.extract::<usize>()
        && let Some(error) = external.into_iter().nth(index).flatten()
    {
        return Err(error);
    }
    Err(config_error(outcome, Some(values)))
}

/// `applied_interpolation`: record the interpolation's template reads, then its SQL or error.
pub(crate) fn applied_interpolation(
    context: &LoopContext<'_, '_>,
    interpolation: &InterpolationRow,
) -> PyResult<String> {
    let InterpolationRow {
        sql, reads, error, ..
    } = interpolation;
    record_reads(context, reads)?;
    match error {
        Some(message) => Err(compile_error(context.classes, message, None)),
        None => Ok(sql.clone()),
    }
}

/// Record interpolation's environment and context reads for compile reuse.
pub(crate) fn record_reads(
    context: &LoopContext<'_, '_>,
    reads: &[(String, String)],
) -> PyResult<()> {
    if !reads.is_empty() {
        let py: Python<'_> = context.host.py();
        context
            .classes
            .config
            .record_template_reads
            .call1((PyTuple::new(py, reads)?,))?;
    }
    Ok(())
}

/// `CompileInputError(message, code=...)`, raised as the attachment raises it.
pub(crate) fn compile_error(classes: &LoopClasses<'_>, message: &str, code: Option<&str>) -> PyErr {
    exception_from(Ok(classes.compile_error.clone()), message, (code, None))
}

/// `CompileInputError(message, help=help)`.
pub(crate) fn helped_compile_error(
    classes: &LoopClasses<'_>,
    message: &str,
    help: Option<&str>,
) -> PyErr {
    exception_from(Ok(classes.compile_error.clone()), message, (None, help))
}

/// The Python exception a native config error raises as, given the values it names.
pub(crate) fn config_error(error: &Bound<'_, PyAny>, values: Option<&Bound<'_, PyDict>>) -> PyErr {
    raised_config_exception(error, values.map(Bound::as_any))
}

/// The spliced SQL's expression macro calls moved through macro expansion into the query's bytes.
pub(crate) fn query_typed_calls(
    calls: Vec<TypedCall>,
    (spliced, expanded, query): (&str, &str, &str),
    macro_spans: &[SpanRow],
) -> Vec<TypedCall> {
    if calls.is_empty() {
        return calls;
    }
    let calls: Vec<TypedCall> = if macro_spans.is_empty() {
        calls
    } else {
        spliced_typed_calls(calls, (spliced, expanded), &spliced_spans(macro_spans))
    };
    if expanded == query {
        return calls;
    }
    let Ok((canonical, spans)) =
        replace_intrinsics_with_spans(expanded, "", |name| format!("{name}()"))
    else {
        return Vec::new();
    };
    if canonical != query {
        return Vec::new();
    }
    let splices: Vec<TextSplice> = spans
        .into_iter()
        .map(|span| TextSplice {
            source: (span.source_start, span.source_end),
            output: (span.output_start, span.output_end),
        })
        .collect();
    moved_typed_calls(calls, &splices)
}

/// Span rows as the spliced spans the scan-fault locator maps through.
pub(crate) fn spliced_spans(rows: &[SpanRow]) -> Vec<SplicedSpan> {
    rows.iter()
        .map(|row| SplicedSpan {
            source_start: row.source_start,
            source_end: row.source_end,
            output_start: row.output_start,
            output_end: row.output_end,
        })
        .collect()
}
