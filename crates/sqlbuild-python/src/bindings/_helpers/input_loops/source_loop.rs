//! `build_source_inputs`: every discovered source attached natively into the compiled project.

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple, PyTupleMethods};
use sqlbuild_analysis::assembly::project::models::{SyntaxCheck, SyntaxMode};
use sqlbuild_attachments::cursor_intrinsics::main::intrinsic_free::intrinsic_free;
use sqlbuild_attachments::cursor_intrinsics::models::IntrinsicCheck;
use sqlbuild_model_config::templates::models::TemplateOptions;

use crate::bindings::_helpers::compiled_project::attachment_inputs::{SourceRow, SourceSettings};
use crate::bindings::_helpers::compiled_views::source_views::{
    AttachedSource, SourceHandles, replaced,
};
use crate::bindings::_helpers::input_loops::model_loop::{
    LoopContext, NativeModelLoop, compile_error, record_reads,
};
use crate::bindings::_helpers::input_loops::model_loop_macros::{
    argument_references as macro_argument_references, expanded_with,
};
use crate::bindings::_helpers::model_config::config_errors::native_config_error;
use crate::bindings::_helpers::model_config::config_errors::raised_config_exception;
use crate::bindings::_helpers::model_config::config_templates::{
    PythonHost, Stop, TemplateSources, expanded, template_error,
};
use crate::bindings::_helpers::project_assembly::resources::{
    syntax_error_message, syntax_error_messages,
};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::render::declaration_expansion::{
    DeclarationExpansion, ExpandedDeclarations, ExpansionClasses, ExpansionConsumer,
    expand_declarations,
};
use crate::bindings::_helpers::render::sql_macro_host::SqlMacroSource;
use crate::bindings::_helpers::scopes::declaration_contexts::NativeDeclarationContexts;
use crate::bindings::_helpers::sqltext::declaration_references::declaration_scan;
use crate::bindings::_helpers::sqltext::static_sql::{InterpolationSources, interpolated};
use sqlbuild_project::models::ReferenceRow;

const MACRO_TOKEN: char = '@';
const NATIVE_CONTEXTS_ATTRIBUTE: &str = "native_contexts";
const GENERIC_DIALECT: &str = "generic";
const EXPRESSION_STATEMENTS: [&str; 3] = ["select", "with", "values"];

/// The SQL parsed for an authored source expression and its syntax error message.
type ParsedExpression = (String, PyResult<Option<String>>);

/// Attach every source in order, as `build_source_inputs` does.
pub(crate) fn attached_sources<'py>(
    model_loop: &NativeModelLoop,
    context: &LoopContext<'_, 'py>,
    rows: Vec<SourceRow>,
    settings: SourceSettings,
) -> PyResult<Vec<AttachedSource>> {
    let (default_contract, validate, source_kind) = settings;
    let py: Python<'py> = context.host.py();
    let source_kind: Bound<'py, PyAny> = source_kind.into_bound(py);
    let mut parsed: Vec<Option<ParsedExpression>> = if validate {
        parsed_expressions(py, &rows)
    } else {
        Vec::new()
    };
    let mut attached: Vec<AttachedSource> = Vec::with_capacity(rows.len());
    for (index, (file_path, source_file, entry)) in rows.into_iter().enumerate() {
        let entry: Bound<'py, PyAny> = entry.into_bound(py);
        let name: String = native_field(&entry, "name")?.extract()?;
        let consumer: Bound<'py, PyAny> = context
            .classes
            .visibility
            .resource_identity
            .call1((&source_kind, &name))?;
        let declarations: Bound<'py, PyAny> = scoped_declarations(context, &file_path, &consumer)?;
        let (expression, usages) =
            match native_field(&entry, "expression")?.extract::<Option<String>>()? {
                Some(sql) => {
                    let expansion: AuthoredExpansion<'py> =
                        expanded_authored_sql(context, (&sql, &file_path), &declarations)?;
                    (Some(expansion.sql), expansion.usages)
                }
                None => (None, PyTuple::empty(py)),
            };
        let label = |field: &str| format!("source {name} {field}");
        let database: Option<String> =
            template_text(context, &entry, "database", &label("database"))?;
        let schema: Option<String> = template_text(context, &entry, "schema", &label("schema"))?;
        let table: Option<String> = template_text(context, &entry, "table", &label("table"))?;
        let description: Option<String> =
            template_text(context, &entry, "description", &label("description"))?;
        let meta: Bound<'py, PyAny> =
            template_value(context, &native_field(&entry, "meta")?, &label("meta"))?;
        let mut columns: Vec<Py<PyAny>> = Vec::new();
        for column in native_field(&entry, "columns")?.try_iter()? {
            columns.push(expanded_column(context, &name, &column?)?);
        }
        let mut audits: Vec<Py<PyAny>> = Vec::new();
        for audit in native_field(&entry, "audits")?.try_iter()? {
            let audit: Bound<'py, PyAny> = audit?;
            let definition: String = native_field(&audit, "definition_name")?.extract()?;
            audits.push(expanded_audit(
                context,
                &audit,
                &format!("source {name} audit {definition}"),
            )?);
        }
        if let Some(sql) = &expression {
            let label: String = format!("Source expression '{name}'");
            if let IntrinsicCheck::Rejected(message) =
                intrinsic_free(sql, &model_loop.reserved_markers, &label)
            {
                return Err(compile_error(context.classes, &message, None));
            }
            if validate {
                let parsed: Option<ParsedExpression> = parsed.get_mut(index).and_then(Option::take);
                checked_expression(context, (&name, &file_path, sql), parsed)?;
            }
        }
        let contract: Option<String> = native_field(&entry, "contract")?
            .extract::<Option<String>>()?
            .or_else(|| default_contract.clone());
        attached.push(AttachedSource {
            name,
            contract,
            database,
            schema,
            table,
            expression,
            description,
            host: SourceHandles {
                source_file,
                entry: entry.unbind(),
                meta: meta.unbind(),
                columns: PyTuple::new(py, columns)?.unbind(),
                audits: PyTuple::new(py, audits)?.unbind(),
                usages: usages.unbind(),
            },
        });
    }
    Ok(attached)
}

/// A source's declaration context, or the public declarations when none are indexed.
pub(crate) fn scoped_declarations<'py>(
    context: &LoopContext<'_, 'py>,
    file_path: &str,
    consumer: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let visibility = &context.classes.visibility;
    if visibility.projects {
        let contexts: Bound<'py, NativeDeclarationContexts> =
            native_field(&visibility.resolver, NATIVE_CONTEXTS_ATTRIBUTE)?.downcast_into()?;
        let (resolved, _) =
            contexts
                .get()
                .resolved_context(&visibility.resolver, file_path, Some(consumer))?;
        return Ok(resolved);
    }
    let fields: Bound<'py, PyDict> = PyDict::new(consumer.py());
    fields.set_item("enums", &visibility.public_enums)?;
    fields.set_item("constants", &visibility.public_constants)?;
    context
        .classes
        .macros
        .declaration_context
        .call((), Some(&fields))
}

/// Authored SQL after expansion, with its usages and typed macro argument references.
pub(crate) struct AuthoredExpansion<'py> {
    pub(crate) sql: String,
    pub(crate) usages: Bound<'py, PyTuple>,
    pub(crate) argument_references: Vec<ReferenceRow>,
}

/// `declaration_file_expansion`: a declaration file's folder context, consumed by `identity`.
pub(crate) fn file_declarations<'py>(
    context: &LoopContext<'_, 'py>,
    file_path: &str,
    identity: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let visibility = &context.classes.visibility;
    let py: Python<'py> = identity.py();
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    fields.set_item("consumer", identity)?;
    if visibility.projects {
        let contexts: Bound<'py, NativeDeclarationContexts> =
            native_field(&visibility.resolver, NATIVE_CONTEXTS_ATTRIBUTE)?.downcast_into()?;
        let (resolved, _) =
            contexts
                .get()
                .resolved_context(&visibility.resolver, file_path, None)?;
        return Ok(replaced(py, &resolved, &fields)?.into_bound(py));
    }
    fields.set_item("enums", &visibility.public_enums)?;
    fields.set_item("constants", &visibility.public_constants)?;
    context
        .classes
        .macros
        .declaration_context
        .call((), Some(&fields))
}

/// `expand_authored_sql_result`: interpolate, splice declarations and expand macros.
pub(crate) fn expanded_authored_sql<'py>(
    context: &LoopContext<'_, 'py>,
    authored: (&str, &str),
    declarations: &Bound<'py, PyAny>,
) -> PyResult<AuthoredExpansion<'py>> {
    let (sql, file_path) = authored;
    let py: Python<'py> = declarations.py();
    let hooks = &context.classes.sql_hooks;
    let (interpolated_sql, reads, error) = interpolated(
        InterpolationSources(
            hooks.effective_vars.clone(),
            hooks.environment.clone(),
            None,
            hooks.render_project_var.clone(),
            Some(context.syntax.clone()),
        ),
        sql,
        file_path,
    )?;
    let reads: Vec<(String, String)> = reads
        .into_iter()
        .map(|(kind, name)| (kind.to_owned(), name))
        .collect();
    record_reads(context, &reads)?;
    if let Some(message) = error {
        return Err(compile_error(context.classes, &message, None));
    }
    let interpolated_sql: String = interpolated_sql.unwrap_or_else(|| sql.to_owned());
    let consumer: Bound<'py, PyAny> = native_field(declarations, "consumer")?;
    let (references, stop) = declaration_scan(&interpolated_sql, Some(context.syntax));
    let mut usages: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut argument_references: Vec<ReferenceRow> = Vec::new();
    let mut expanded_sql: String = interpolated_sql.clone();
    if !references.is_empty() || stop.is_some() {
        let classes: ExpansionClasses<'py> = ExpansionClasses {
            usage_record: context.classes.usage_record.clone(),
            compile_error: context.classes.compile_error.clone(),
        };
        let expanded: ExpandedDeclarations<'py> = expand_declarations(&DeclarationExpansion {
            sql: &interpolated_sql,
            references: &references,
            stop,
            context: declarations,
            consumer: ExpansionConsumer {
                identity: (!consumer.is_none()).then(|| consumer.clone()),
                relationship_visibility: false,
                path: file_path,
            },
            rendering: context.host.get(),
            classes: &classes,
            sql_macros: Some(SqlMacroSource {
                loaded_macros: Some(context.classes.macros.loaded_macros.bind(py).as_any()),
                origin: None,
                scopes: Some(&context.classes.visibility),
                syntax: Some(context.syntax),
            }),
        })?;
        expanded_sql = expanded.spliced.sql;
        usages.extend(expanded.usages);
    }
    if expanded_sql.contains(MACRO_TOKEN) {
        let resolver: &Bound<'py, PyAny> = &context.classes.visibility.resolver;
        let macro_declarations: Bound<'py, PyAny> = if resolver.is_none() {
            py.None().into_bound(py)
        } else {
            let contexts: Bound<'py, NativeDeclarationContexts> =
                native_field(resolver, NATIVE_CONTEXTS_ATTRIBUTE)?.downcast_into()?;
            contexts
                .get()
                .resolved_context(resolver, file_path, None)?
                .0
        };
        let output = expanded_with(
            context,
            (&macro_declarations, &consumer),
            file_path,
            &expanded_sql,
        )?;
        argument_references = macro_argument_references(context, &output)?;
        usages.extend(output.usages.into_iter().map(|usage| usage.into_bound(py)));
        expanded_sql = output.sql;
    }
    let unique: Bound<'py, PyDict> = PyDict::new(py);
    for usage in usages {
        unique.set_item(usage, py.None())?;
    }
    Ok(AuthoredExpansion {
        sql: expanded_sql,
        usages: PyTuple::new(py, unique.keys())?,
        argument_references,
    })
}

/// `_expand_source_template_value`: a text field with its `${...}` templates expanded.
fn template_text(
    context: &LoopContext<'_, '_>,
    owner: &Bound<'_, PyAny>,
    field: &str,
    label: &str,
) -> PyResult<Option<String>> {
    let raw: Bound<'_, PyAny> = native_field(owner, field)?;
    if raw.is_none() {
        return Ok(None);
    }
    Ok(Some(
        template_value(context, &raw, label)?.str()?.extract()?,
    ))
}

/// `_expand_source_template_object`: `${...}` templates expanded without context values.
fn template_value<'py>(
    context: &LoopContext<'_, 'py>,
    value: &Bound<'py, PyAny>,
    label: &str,
) -> PyResult<Bound<'py, PyAny>> {
    expanded_template(context, value, label, false)
}

/// `expand_config_templates` without context values; `preserve` keeps `@@CTX` tokens.
pub(crate) fn expanded_template<'py>(
    context: &LoopContext<'_, 'py>,
    value: &Bound<'py, PyAny>,
    label: &str,
    preserve: bool,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = value.py();
    let hooks = &context.classes.sql_hooks;
    let host: PythonHost<'py> = PythonHost::new(
        py,
        TemplateSources(
            hooks.effective_vars.clone(),
            hooks.environment.clone(),
            PyDict::new(py),
        ),
        label,
    );
    let options: TemplateOptions = TemplateOptions {
        allow_context: false,
        preserve_context_tokens: preserve,
        preserve_unknown_context: false,
    };
    let result: Result<Bound<'py, PyAny>, Stop> = expanded(&host, value, options);
    let reads: Vec<(String, String)> = host
        .into_reads()
        .into_iter()
        .map(|(kind, name)| (kind.to_owned(), name))
        .collect();
    record_reads(context, &reads)?;
    match result {
        Ok(value) => Ok(value),
        Err(Stop::Python(error)) => Err(error),
        Err(Stop::Failure(error)) => {
            let error: Bound<'py, PyAny> = native_config_error(py, template_error(&error, label))?
                .into_bound(py)
                .into_any();
            Err(raised_config_exception(&error, None))
        }
    }
}

/// `expand_source_column_templates`.
fn expanded_column(
    context: &LoopContext<'_, '_>,
    source: &str,
    column: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = column.py();
    let column_name: String = native_field(column, "name")?.str()?.extract()?;
    let label = |field: &str| format!("source {source} column {column_name} {field}");
    let changes: Bound<'_, PyDict> = PyDict::new(py);
    changes.set_item(
        "type",
        template_text(context, column, "type", &label("type"))?,
    )?;
    changes.set_item(
        "description",
        template_text(context, column, "description", &label("description"))?,
    )?;
    changes.set_item(
        "meta",
        template_value(context, &native_field(column, "meta")?, &label("meta"))?,
    )?;
    let mut audits: Vec<Py<PyAny>> = Vec::new();
    for audit in native_field(column, "audits")?.try_iter()? {
        let audit: Bound<'_, PyAny> = audit?;
        let definition: String = native_field(&audit, "definition_name")?.extract()?;
        audits.push(expanded_audit(
            context,
            &audit,
            &label(&format!("audit {definition}")),
        )?);
    }
    changes.set_item("audits", PyTuple::new(py, audits)?)?;
    replaced_spec(column, &changes)
}

/// `expand_schema_audit_instance_templates`.
fn expanded_audit(
    context: &LoopContext<'_, '_>,
    audit: &Bound<'_, PyAny>,
    label: &str,
) -> PyResult<Py<PyAny>> {
    let changes: Bound<'_, PyDict> = PyDict::new(audit.py());
    changes.set_item(
        "arguments",
        template_value(
            context,
            &native_field(audit, "arguments")?,
            &format!("{label} arguments"),
        )?,
    )?;
    changes.set_item(
        "description",
        template_text(
            context,
            audit,
            "description",
            &format!("{label} description"),
        )?,
    )?;
    replaced_spec(audit, &changes)
}

/// `dataclasses.replace(value, **changes)`, or `value` itself when every change keeps its field.
fn replaced_spec(value: &Bound<'_, PyAny>, changes: &Bound<'_, PyDict>) -> PyResult<Py<PyAny>> {
    for (name, changed) in changes.iter() {
        let current: Bound<'_, PyAny> = native_field(value, &name.extract::<String>()?)?;
        if !same_value(&current, &changed)? {
            return replaced(value.py(), value, changes);
        }
    }
    Ok(value.clone().unbind())
}

/// Whether `changed` is `current` or an equal value of the same type, element by element.
fn same_value(current: &Bound<'_, PyAny>, changed: &Bound<'_, PyAny>) -> PyResult<bool> {
    if current.is(changed) {
        return Ok(true);
    }
    if !current.get_type().is(changed.get_type()) {
        return Ok(false);
    }
    if let (Ok(current), Ok(changed)) =
        (current.downcast::<PyTuple>(), changed.downcast::<PyTuple>())
    {
        if current.len() != changed.len() {
            return Ok(false);
        }
        for (left, right) in current.iter().zip(changed.iter()) {
            if !left.is(&right) {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    current.eq(changed)
}

/// `validate_source_expression_syntax`: the expression parses as a FROM target.
fn checked_expression(
    context: &LoopContext<'_, '_>,
    source: (&str, &str, &str),
    parsed: Option<ParsedExpression>,
) -> PyResult<()> {
    let (name, file_path, expression) = source;
    let sql: String = expression_sql(expression);
    let error_prefix: String =
        format!("SQL syntax error in source expression '{name}' ({file_path})");
    match parsed {
        Some((parsed_sql, message)) if parsed_sql == sql => match message? {
            Some(message) => Err(syntax_error(context, &error_prefix, &message)),
            None => Ok(()),
        },
        _ => parse_checked(context, &sql, &error_prefix),
    }
}

/// The SQL `validate_source_expression_syntax` parses for `expression`.
fn expression_sql(expression: &str) -> String {
    format!("SELECT * FROM {}", expression_relation(expression))
}

/// Each authored source expression's parse, run together ahead of the ordered loop.
fn parsed_expressions(py: Python<'_>, rows: &[SourceRow]) -> Vec<Option<ParsedExpression>> {
    let mut positions: Vec<Option<usize>> = Vec::with_capacity(rows.len());
    let mut checks: Vec<SyntaxCheck> = Vec::new();
    for (_, _, entry) in rows {
        let mut position: Option<usize> = None;
        if let Ok(Some(expression)) = native_field(entry.bind(py), "expression")
            .and_then(|value| value.extract::<Option<String>>())
        {
            position = Some(checks.len());
            checks.push(SyntaxCheck {
                sql: expression_sql(&expression),
                placeholders: Vec::new(),
            });
        }
        positions.push(position);
    }
    let mut messages: Vec<Option<PyResult<Option<String>>>> =
        syntax_error_messages(py, &checks, GENERIC_DIALECT, SyntaxMode::Parse)
            .into_iter()
            .map(Some)
            .collect();
    positions
        .into_iter()
        .map(|position| {
            let index: usize = position?;
            let message: PyResult<Option<String>> = messages[index].take()?;
            Some((std::mem::take(&mut checks[index].sql), message))
        })
        .collect()
}

/// `_validate_sql_syntax_with_message` for SQL a non-model resource owns.
pub(crate) fn parse_checked(
    context: &LoopContext<'_, '_>,
    sql: &str,
    error_prefix: &str,
) -> PyResult<()> {
    let py: Python<'_> = context.host.py();
    let check: SyntaxCheck = SyntaxCheck {
        sql: sql.to_owned(),
        placeholders: Vec::new(),
    };
    let Some(message) = syntax_error_message(py, &check, GENERIC_DIALECT, SyntaxMode::Parse)?
    else {
        return Ok(());
    };
    Err(syntax_error(context, error_prefix, &message))
}

/// The compile error raised for a non-model resource's SQL that does not parse.
fn syntax_error(context: &LoopContext<'_, '_>, error_prefix: &str, message: &str) -> PyErr {
    let py: Python<'_> = context.host.py();
    let keywords: Bound<'_, PyDict> = PyDict::new(py);
    let error: PyResult<Bound<'_, PyAny>> = keywords
        .set_item("help", &context.classes.source_syntax_help)
        .and_then(|()| {
            context
                .classes
                .compile_error
                .call((format!("{error_prefix}: {message}"),), Some(&keywords))
        });
    match error {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

/// `_render_source_expression_relation`: the expression as a table factor.
fn expression_relation(expression: &str) -> String {
    let trimmed: &str = str::trim(expression);
    let stripped: &str = str::trim(trimmed.strip_suffix(';').unwrap_or(trimmed));
    if stripped.starts_with('(') {
        return stripped.to_owned();
    }
    let lowered: String = stripped.to_lowercase();
    if EXPRESSION_STATEMENTS
        .iter()
        .any(|statement| lowered.starts_with(statement))
    {
        return format!("({stripped})");
    }
    stripped.to_owned()
}
