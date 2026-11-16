//! Authored SQL expanded through project vars, declarations and macros, with each pass's spans.

use crate::render::main::calls::span_row::span_row;
use crate::render::main::calls::span_tuple::span_tuple;
use crate::render::main::declarations::expand_with_inputs::expand_with_inputs;
use crate::render::main::declarations::span_rows::span_rows;
use crate::render::models::AuthoredExpansion;
use crate::render::models::SqlExpansionInputs;
use pyo3::prelude::{Bound, IntoPyObject, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::model_config::main::config::record_reads::record_reads;
use crate::model_config::main::model_nodes::unicode_environment::unicode_environment;
use crate::render::_helpers::declarations::declaration_expansion::expand_declarations;
use crate::render::_helpers::declarations::declaration_values::resolved_declaration_context;
use crate::render::_helpers::expansion::macro_expansion_result::macro_output;
use crate::render::models::ConstantRendering;
use crate::render::models::DeclarationExpansionClasses;
use crate::render::models::ExpansionRequest;
use crate::render::models::SqlMacroSource;
use crate::render::models::{DeclarationExpansion, ExpandedDeclarations, ExpansionConsumer};
use crate::render::types::SpanRow;
use crate::sqltext::main::declaration_scan::declaration_scan;
use crate::sqltext::main::interpolation_batch::interpolation_batch;
use crate::sqltext::main::optional_record_syntax::optional_record_syntax;
use crate::sqltext::models::{InterpolationRow, InterpolationSources};
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::construct::new_record::new_record;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::registry::native_attr::native_attr;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::records::types::NativeAttr;
use sqlbuild_python_base::records::types::RecordId;

/// The macro context field holding the project's `SqlLexicalSyntax`.
const LEXICAL_SYNTAX_FIELD: &str = "_lexical_syntax";
const DECLARATION_FIELDS: [&str; 4] = [
    "enums",
    "constants",
    "inaccessible_enums",
    "inaccessible_constants",
];

/// The project's lexical rules the macro context holds, if any.
fn context_syntax(macro_context: &Bound<'_, PyAny>) -> PyResult<Option<LexicalSyntax>> {
    if macro_context.is_none() {
        return Ok(None);
    }
    optional_record_syntax(&native_field(macro_context, LEXICAL_SYNTAX_FIELD)?)
}

/// `sql` with `@@` tokens interpolated and each substitution's span, raising its error.
fn substituted<'py>(
    sql: &Bound<'py, PyAny>,
    file_path: &Bound<'py, PyAny>,
    (effective_vars, context_values): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    syntax: Option<&LexicalSyntax>,
) -> PyResult<(String, Vec<SpanRow>)> {
    let py: Python<'py> = sql.py();
    let sources = InterpolationSources(
        effective_vars.downcast()?.clone(),
        unicode_environment(py)?,
        if context_values.is_none() {
            None
        } else {
            Some(context_values.downcast()?.clone())
        },
        native_attr(py, NativeAttr::RenderProjectVarText)?,
        syntax.cloned(),
    );
    let text: String = sql.extract()?;
    let path: String = file_path.str()?.extract()?;
    let rows = interpolation_batch(sources, &[(text.clone(), path)])?;
    let Some(InterpolationRow {
        sql: rendered,
        spans,
        reads,
        error,
    }) = rows.into_iter().next()
    else {
        return Ok((text, Vec::new()));
    };
    record_reads(py, reads.iter().map(|(kind, name)| (*kind, name.as_str())))?;
    if let Some(message) = error {
        return Err(PyErr::from_value(
            error_class(py, ErrorId::CompileInput)?.call1((message,))?,
        ));
    }
    Ok((rendered.unwrap_or(text), spans))
}

/// `ExpansionSpan` objects for span rows, in order.
fn span_objects<'py>(py: Python<'py>, spans: &[SpanRow]) -> PyResult<Bound<'py, PyTuple>> {
    let objects: Vec<Bound<'py, PyAny>> = spans
        .iter()
        .map(|span| {
            new_record(
                py,
                RecordId::ExpansionSpan,
                IntoPyObject::into_pyobject(span_tuple(span), py)?,
                None,
            )
        })
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, objects)
}

/// The authored SQL's public declaration context: visible and inaccessible enums and constants.
fn declaration_context<'py>(
    py: Python<'py>,
    declarations: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let fields: Vec<(&str, Bound<'py, PyAny>)> = DECLARATION_FIELDS
        .iter()
        .map(|field| {
            Ok((
                *field,
                if declarations.is_none() {
                    PyDict::new(py).into_any()
                } else {
                    native_field(declarations, field)?
                },
            ))
        })
        .collect::<PyResult<_>>()?;
    new_record_with(py, RecordId::DeclarationResolutionContext, &fields)
}

/// `sql` with natively scanned enum and constant references spliced, and their spans.
fn declarations_expanded<'py>(
    sql: &str,
    (file_path, loaded_macros): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    (declarations, value_renderer, collection_rendering): (
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
        &Bound<'py, PyAny>,
    ),
    syntax: Option<&LexicalSyntax>,
) -> PyResult<(String, Vec<SpanRow>)> {
    let py: Python<'py> = file_path.py();
    let (references, stop) = declaration_scan(sql, syntax);
    if references.is_empty() && stop.is_none() {
        return Ok((sql.to_owned(), Vec::new()));
    }
    let context: Bound<'py, PyAny> = declaration_context(py, declarations)?;
    let rendering: ConstantRendering = ConstantRendering::new(
        value_renderer.clone().unbind(),
        collection_rendering.clone().unbind(),
    );
    let classes = DeclarationExpansionClasses {
        usage_record: record_class(py, RecordId::UsageRecord)?,
        compile_error: error_class(py, ErrorId::CompileInput)?,
    };
    let path: String = file_path.str()?.extract()?;
    let expanded: ExpandedDeclarations<'py> = expand_declarations(&DeclarationExpansion {
        sql,
        references: &references,
        stop,
        context: &context,
        consumer: ExpansionConsumer {
            identity: None,
            relationship_visibility: false,
            path: &path,
        },
        rendering: &rendering,
        classes: &classes,
        sql_macros: Some(SqlMacroSource {
            loaded_macros: Some(loaded_macros),
            origin: None,
            scopes: None,
            syntax,
        }),
    })?;
    let spans: Vec<SpanRow> = span_rows(&expanded.spliced);
    Ok((expanded.spliced.sql, spans))
}

/// What one authored SQL expansion reads.
pub(crate) struct AuthoredSources<'a, 'py> {
    pub(crate) sql: &'a Bound<'py, PyAny>,
    pub(crate) file_path: &'a Bound<'py, PyAny>,
    pub(crate) effective_vars: &'a Bound<'py, PyAny>,
    pub(crate) context_values: &'a Bound<'py, PyAny>,
    pub(crate) declarations: &'a Bound<'py, PyAny>,
    pub(crate) value_renderer: &'a Bound<'py, PyAny>,
    pub(crate) collection_rendering: &'a Bound<'py, PyAny>,
    pub(crate) loaded_macros: &'a Bound<'py, PyAny>,
    pub(crate) macro_context: &'a Bound<'py, PyAny>,
    pub(crate) declaration_resolver: Option<Bound<'py, PyAny>>,
}

pub(crate) fn authored_expansion(sources: AuthoredSources<'_, '_>) -> PyResult<AuthoredExpansion> {
    let py: Python<'_> = sources.sql.py();
    let syntax: Option<LexicalSyntax> = context_syntax(sources.macro_context)?;
    let (interpolated, interpolation_spans) = substituted(
        sources.sql,
        sources.file_path,
        (sources.effective_vars, sources.context_values),
        syntax.as_ref(),
    )?;
    let (declared, declaration_spans) = declarations_expanded(
        &interpolated,
        (sources.file_path, sources.loaded_macros),
        (
            sources.declarations,
            sources.value_renderer,
            sources.collection_rendering,
        ),
        syntax.as_ref(),
    )?;
    let request: ExpansionRequest<'_> = ExpansionRequest {
        sql: PyString::new(py, &declared),
        file_path: sources.file_path.clone(),
        loaded_macros: sources.loaded_macros.downcast()?.clone(),
        macro_overrides: None,
        macro_context: sources.macro_context.clone(),
        declaration_resolver: sources.declaration_resolver,
        declarations: None,
        consumer: None,
    };
    Ok(match macro_output(request)? {
        Some(output) => AuthoredExpansion {
            sql: output.sql,
            passes: [
                interpolation_spans,
                declaration_spans,
                output.spans.into_iter().map(span_row).collect(),
            ],
        },
        None => AuthoredExpansion {
            sql: declared,
            passes: [interpolation_spans, declaration_spans, Vec::new()],
        },
    })
}

/// `(sql, passes)` with each pass's `ExpansionSpan` objects.
fn expansion_tuple<'py>(
    py: Python<'py>,
    expansion: &AuthoredExpansion,
) -> PyResult<Bound<'py, PyTuple>> {
    let passes: Vec<Bound<'py, PyTuple>> = expansion
        .passes
        .iter()
        .map(|spans| span_objects(py, spans))
        .collect::<PyResult<_>>()?;
    PyTuple::new(
        py,
        [
            PyString::new(py, &expansion.sql).into_any(),
            PyTuple::new(py, passes)?.into_any(),
        ],
    )
}

/// Expand authored SQL, returning each pass's substitution spans in order.
fn authored_sql_with_spans<'py>(request: &Request<'_, 'py>) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = request.py();
    let none = || py.None().into_bound(py);
    let expansion: AuthoredExpansion = authored_expansion(AuthoredSources {
        sql: &request.get("sql")?,
        file_path: &request.get("file_path")?,
        effective_vars: &request.get("effective_vars")?,
        context_values: &request.optional("context_values")?.unwrap_or_else(none),
        declarations: &request.optional("declarations")?.unwrap_or_else(none),
        value_renderer: &request.get("value_renderer")?,
        collection_rendering: &request.get("collection_rendering")?,
        loaded_macros: &request.get("loaded_macros")?,
        macro_context: &request.get("macro_context")?,
        declaration_resolver: request
            .optional("declaration_resolver")?
            .filter(|resolver| !resolver.is_none()),
    })?;
    expansion_tuple(py, &expansion)
}

/// Expand authored SQL, returning each pass's substitution spans in order.
#[pyfunction]
#[pyo3(signature = (**request))]
pub(crate) fn expand_authored_sql_with_spans<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    authored_sql_with_spans(&Request::new("expand_authored_sql_with_spans", request)?)
}

/// The declarations visible to `file_path`: the resolver's, else the inputs', plus model-local.
pub(crate) fn input_declarations<'py>(
    inputs: &SqlExpansionInputs,
    file_path: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = file_path.py();
    let resolver: &Bound<'py, PyAny> = inputs.declaration_resolver.bind(py);
    let declarations: Bound<'py, PyAny> = if resolver.is_none() {
        new_record_with(
            py,
            RecordId::DeclarationResolutionContext,
            &[
                ("enums", inputs.enums.bind(py).clone()),
                ("constants", inputs.constants.bind(py).clone()),
            ],
        )?
    } else {
        resolved_declaration_context(resolver, file_path, None)?
    };
    let Some(local) = inputs.local_declarations.bind(py).get_item(file_path)? else {
        return Ok(declarations);
    };
    if local.is_none() {
        return Ok(declarations);
    }
    let merged = |field: &str| -> PyResult<Bound<'py, PyAny>> {
        native_field(&declarations, field)?.bitor(native_field(&local, field)?)
    };
    new_record_with(
        py,
        RecordId::DeclarationResolutionContext,
        &[
            ("enums", merged("enums")?),
            ("constants", merged("constants")?),
            (
                "inaccessible_enums",
                native_field(&declarations, "inaccessible_enums")?,
            ),
            (
                "inaccessible_constants",
                native_field(&declarations, "inaccessible_constants")?,
            ),
        ],
    )
}

/// Expand authored SQL with a project's expansion context and report each pass's spans.
#[pyfunction]
#[pyo3(signature = (*, sql, file_path, context))]
pub(crate) fn expand_sql_with_spans<'py>(
    sql: &str,
    file_path: &Bound<'py, PyAny>,
    context: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let inputs: SqlExpansionInputs = SqlExpansionInputs::from_record(context)?;
    expansion_tuple(
        file_path.py(),
        &expand_with_inputs(&inputs, sql, file_path)?,
    )
}

/// Replace SQL interpolation tokens, returning the span of every substitution.
#[pyfunction]
#[pyo3(signature = (*, sql, file_path, effective_vars, context_values=None))]
pub(crate) fn substitute_sql_vars_with_spans<'py>(
    sql: &Bound<'py, PyAny>,
    file_path: &Bound<'py, PyAny>,
    effective_vars: &Bound<'py, PyAny>,
    context_values: Option<Bound<'py, PyAny>>,
) -> PyResult<(String, Bound<'py, PyTuple>)> {
    let py: Python<'py> = sql.py();
    let context_values: Bound<'py, PyAny> =
        context_values.unwrap_or_else(|| py.None().into_bound(py));
    let (substituted_sql, spans) =
        substituted(sql, file_path, (effective_vars, &context_values), None)?;
    Ok((substituted_sql, span_objects(py, &spans)?))
}
