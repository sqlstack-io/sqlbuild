//! Authored SQL expanded through project vars, declarations and macros, with each pass's spans.

use pyo3::prelude::Py;
use pyo3::prelude::{
    Bound, IntoPyObject, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::bindings::_helpers::boundary::keyword_request::Request;
use crate::bindings::_helpers::model_config::template_data::record_reads;
use crate::bindings::_helpers::model_config::unicode_environment::unicode_environment;
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::construct::{new_record, record_class};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::records::ids::{ErrorId, NativeAttr};
use crate::bindings::_helpers::records::registry;
use crate::bindings::_helpers::render::declaration_expansion::{
    DeclarationExpansion, ExpandedDeclarations, ExpansionClasses, ExpansionConsumer,
    expand_declarations, span_rows,
};
use crate::bindings::_helpers::render::declaration_values::ConstantRendering;
use crate::bindings::_helpers::render::declaration_values::resolved_declaration_context;
use crate::bindings::_helpers::render::macro_call_rows::SpanRow;
use crate::bindings::_helpers::render::macro_call_rows::{span_row, span_tuple};
use crate::bindings::_helpers::render::macro_expansion_result::{ExpansionRequest, macro_output};
use crate::bindings::_helpers::render::sql_macro_host::SqlMacroSource;
use crate::bindings::_helpers::sqltext::declaration_references::declaration_scan;
use crate::bindings::_helpers::sqltext::lexical_syntax::optional_record_syntax;
use crate::bindings::_helpers::sqltext::static_sql::{
    InterpolationRow, InterpolationSources, interpolation_batch,
};

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
        registry::native(py, NativeAttr::RenderProjectVarText)?,
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
    let classes = ExpansionClasses {
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

/// Authored SQL expanded through vars, declarations and macros, with each pass's spans.
pub(crate) struct AuthoredExpansion {
    pub(crate) sql: String,
    /// Interpolation, declaration and macro spans, in pass order.
    pub(crate) passes: [Vec<SpanRow>; 3],
}

/// What one authored SQL expansion reads.
struct AuthoredSources<'a, 'py> {
    sql: &'a Bound<'py, PyAny>,
    file_path: &'a Bound<'py, PyAny>,
    effective_vars: &'a Bound<'py, PyAny>,
    context_values: &'a Bound<'py, PyAny>,
    declarations: &'a Bound<'py, PyAny>,
    value_renderer: &'a Bound<'py, PyAny>,
    collection_rendering: &'a Bound<'py, PyAny>,
    loaded_macros: &'a Bound<'py, PyAny>,
    macro_context: &'a Bound<'py, PyAny>,
    declaration_resolver: Option<Bound<'py, PyAny>>,
}

fn authored_expansion(sources: AuthoredSources<'_, '_>) -> PyResult<AuthoredExpansion> {
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

/// `sql` expanded with lint's expansion inputs, as `file_path`'s declarations see them.
pub(crate) fn expand_with_inputs(
    inputs: &SqlExpansionInputs,
    sql: &str,
    file_path: &Bound<'_, PyAny>,
) -> PyResult<AuthoredExpansion> {
    let py: Python<'_> = file_path.py();
    let resolver: Bound<'_, PyAny> = inputs.declaration_resolver.bind(py).clone();
    authored_expansion(AuthoredSources {
        sql: &PyString::new(py, sql).into_any(),
        file_path,
        effective_vars: inputs.effective_vars.bind(py),
        context_values: &py.None().into_bound(py),
        declarations: &input_declarations(inputs, file_path)?,
        value_renderer: inputs.value_renderer.bind(py),
        collection_rendering: inputs.collection_rendering.bind(py),
        loaded_macros: inputs.loaded_macros.bind(py),
        macro_context: inputs.macro_context.bind(py),
        declaration_resolver: (!resolver.is_none()).then_some(resolver),
    })
}

/// Expand authored SQL, returning each pass's substitution spans in order.
#[pyfunction]
#[pyo3(signature = (**request))]
fn expand_authored_sql_with_spans<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    authored_sql_with_spans(&Request::new("expand_authored_sql_with_spans", request)?)
}

/// The declarations visible to `file_path`: the resolver's, else the inputs', plus model-local.
fn input_declarations<'py>(
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
fn expand_sql_with_spans<'py>(
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
fn substitute_sql_vars_with_spans<'py>(
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

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(expand_authored_sql_with_spans, module)?)?;
    module.add_function(wrap_pyfunction!(expand_sql_with_spans, module)?)?;
    module.add_function(wrap_pyfunction!(substitute_sql_vars_with_spans, module)?)?;
    Ok(())
}

/// The inputs authored SQL is expanded with outside a compile; the handles carry user values.
pub(crate) struct SqlExpansionInputs {
    pub(crate) effective_vars: Py<PyAny>,
    pub(crate) loaded_macros: Py<PyAny>,
    pub(crate) macro_context: Py<PyAny>,
    pub(crate) enums: Py<PyAny>,
    pub(crate) constants: Py<PyAny>,
    pub(crate) value_renderer: Py<PyAny>,
    pub(crate) collection_rendering: Py<PyAny>,
    /// Each model file's own declarations, by file path.
    pub(crate) local_declarations: Py<PyDict>,
    pub(crate) declaration_resolver: Py<PyAny>,
}

impl SqlExpansionInputs {
    /// The inputs a `SqlExpansionContext` record holds.
    pub(crate) fn from_record(record: &Bound<'_, PyAny>) -> PyResult<Self> {
        let field =
            |name: &str| -> PyResult<Py<PyAny>> { Ok(native_field(record, name)?.unbind()) };
        Ok(Self {
            effective_vars: field("effective_vars")?,
            loaded_macros: field("loaded_macros")?,
            macro_context: field("macro_context")?,
            enums: field("enums")?,
            constants: field("constants")?,
            value_renderer: field("value_renderer")?,
            collection_rendering: field("collection_rendering")?,
            local_declarations: native_field(record, "local_declarations")?
                .downcast_into::<PyDict>()?
                .unbind(),
            declaration_resolver: field("declaration_resolver")?,
        })
    }

    /// These inputs as a `SqlExpansionContext` record.
    pub(crate) fn record<'py>(&self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        new_record_with(
            py,
            RecordId::SqlExpansionContext,
            &[
                ("effective_vars", self.effective_vars.bind(py).clone()),
                ("loaded_macros", self.loaded_macros.bind(py).clone()),
                ("macro_context", self.macro_context.bind(py).clone()),
                ("enums", self.enums.bind(py).clone()),
                ("constants", self.constants.bind(py).clone()),
                ("value_renderer", self.value_renderer.bind(py).clone()),
                (
                    "collection_rendering",
                    self.collection_rendering.bind(py).clone(),
                ),
                (
                    "local_declarations",
                    self.local_declarations.bind(py).clone().into_any(),
                ),
                (
                    "declaration_resolver",
                    self.declaration_resolver.bind(py).clone(),
                ),
            ],
        )
    }
}
