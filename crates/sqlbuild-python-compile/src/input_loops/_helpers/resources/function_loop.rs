//! `build_sql_function_inputs`: every SQL and Python function attached natively.

use std::collections::{HashMap, HashSet};

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyStringMethods, PyTuple};
use sqlbuild_analysis::assembly::analysis_session::main::query_columns::query_columns;
use sqlbuild_analysis::assembly::analysis_session::models::{
    QueryColumns, QueryColumnsInput, QueryColumnsMode, QueryColumnsRequest,
};
use sqlbuild_analysis::assembly::project::errors::AssemblyError;
use sqlbuild_analysis::assembly::project::main::preserved_logical_namespace::preserved_logical_namespace;
use sqlbuild_analysis::assembly::project::models::TargetNamespace;
use sqlbuild_analysis::compiled_project::main::located_scan_error::located_scan_error;
use sqlbuild_analysis::compiled_project::main::merge_call_site_references::merge_call_site_references;
use sqlbuild_analysis::compiled_project::models::ScanFaultOrigin;
use sqlbuild_analysis::compiler::main::function_type_validation::function_type_error;
use sqlbuild_attachments::cursor_intrinsics::main::intrinsic_free::intrinsic_free;
use sqlbuild_attachments::cursor_intrinsics::models::IntrinsicCheck;
use sqlbuild_attachments::functions::main::function_reference_error::function_reference_error;
use sqlbuild_attachments::functions::main::resolve_function_namespace::resolve_function_namespace;
use sqlbuild_attachments::functions::main::table_function_arity_error::table_function_arity_error;
use sqlbuild_attachments::functions::models::{
    FunctionHeader, FunctionLanguage, FunctionNamespace, FunctionReturns, HeaderStage,
    KnownResources, NamedType, NamespaceInputs,
};
use sqlbuild_sqltext::sql_references::main::extract_sql_references::extract_sql_references;
use sqlbuild_sqltext::sql_references::models::ReferenceExtraction;

use crate::input_loops::_helpers::model::model_loop::LoopContext;
use crate::input_loops::_helpers::model::model_loop::compile_error;
use crate::input_loops::_helpers::model::model_loop_calls::CallOrigin;
use crate::input_loops::_helpers::model::model_loop_calls::report_invalid_calls;
use crate::input_loops::_helpers::model::model_loop_inputs::reference_row;
use crate::input_loops::_helpers::resources::source_loop::AuthoredExpansion;
use crate::input_loops::_helpers::resources::source_loop::expanded_authored_sql;
use crate::input_loops::_helpers::resources::source_loop::expanded_template;
use crate::input_loops::_helpers::resources::source_loop::parse_checked;
use crate::input_loops::_helpers::resources::source_loop::scoped_declarations;
use crate::input_loops::models::NativeModelLoop;
use sqlbuild_project::models::DestinationFact;
use sqlbuild_project::models::PythonRuntime;
use sqlbuild_project::models::ReferenceRow;
use sqlbuild_project::types::NamedTypeRow;
use sqlbuild_python_base::compile_context::main::delta::record_source_reads::record_source_reads;
use sqlbuild_python_front::attachments::main::parsed_header::parsed_header;
use sqlbuild_python_front::model_config::models::NativeModelValidator;
use sqlbuild_python_project::analysis_session::main::query_columns_error::query_columns_error;
use sqlbuild_python_project::compiled_project::models::{FunctionRow, FunctionSettings};
use sqlbuild_python_project::resource_views::models::FunctionHandles;
use sqlbuild_python_project::resource_views::types::AttachedFunction;

const PRESERVE_TARGET_VALUE: &str = "preserve";
const GENERIC_DIALECT: &str = "generic";
const TABLE_RETURNS: &str = "TABLE";
const DATABASE_KEY: &str = "database";
const SCHEMA_KEY: &str = "schema";

/// The reference kind of a source.
const SOURCE_KIND: &str = "source";

/// What every function in one compile resolves against.
struct FunctionScope<'a, 'py> {
    model_loop: &'a NativeModelLoop,
    context: &'a LoopContext<'a, 'py>,
    adapter_name: String,
    validate: bool,
    defaults: (Option<String>, Option<String>),
    targets: (Option<String>, Option<String>),
    target: Option<TargetNamespace>,
    inherit: bool,
    function_kind: Bound<'py, PyAny>,
}

/// Attach every SQL function, then every Python function, in discovery order.
pub(crate) fn attached_functions<'py>(
    model_loop: &NativeModelLoop,
    context: &LoopContext<'_, 'py>,
    rows: (Vec<FunctionRow>, Vec<FunctionRow>),
    settings: FunctionSettings,
) -> PyResult<Vec<AttachedFunction>> {
    let py: Python<'py> = context.host.py();
    let FunctionSettings {
        adapter_name,
        validate,
        defaults: (default_database, default_schema),
        target,
        inherit,
        kind,
    } = settings;
    let defaults = (
        optional_template(context, default_database, "default function database")?,
        optional_template(context, default_schema, "default function schema")?,
    );
    let targets = match &target {
        Some((database, schema)) => (
            environment_value(context, database.as_deref(), "target database")?,
            environment_value(context, schema.as_deref(), "target schema")?,
        ),
        None => (None, None),
    };
    let scope = FunctionScope {
        model_loop,
        context,
        adapter_name,
        validate,
        defaults,
        targets,
        target: target.map(|(database, schema)| TargetNamespace {
            database,
            schema,
            loader_schema: None,
        }),
        inherit,
        function_kind: kind.into_bound(py),
    };
    let (sql_rows, python_rows) = rows;
    let mut names: HashSet<String> = HashSet::new();
    let mut attached: Vec<AttachedFunction> =
        Vec::with_capacity(sql_rows.len() + python_rows.len());
    for row in sql_rows {
        if !names.insert(row.name.clone()) {
            let message: String = format!("Duplicate SQL function name '{}'", row.name);
            return Err(compile_error(context.classes, &message, None));
        }
        attached.push(sql_function(&scope, row)?);
    }
    for row in python_rows {
        if !names.insert(row.name.clone()) {
            let message: String = format!("Duplicate function name '{}'", row.name);
            return Err(compile_error(context.classes, &message, None));
        }
        attached.push(python_function(&scope, row)?);
    }
    Ok(attached)
}

/// One SQL function, raising each header error where Python's attachment raises it.
fn sql_function(scope: &FunctionScope<'_, '_>, row: FunctionRow) -> PyResult<AttachedFunction> {
    let context: &LoopContext<'_, '_> = scope.context;
    let py: Python<'_> = context.host.py();
    let FunctionRow {
        file_path,
        relative_path,
        name,
        function_file,
        header_values,
        body,
        contents,
    } = row;
    let header_values: Bound<'_, PyDict> = header_values.into_bound(py);
    let label: String = format!("SQL function {relative_path}");
    let header: FunctionHeader =
        parsed_header(&header_values, FunctionLanguage::Sql, &relative_path);
    raise_stage(context, &header, HeaderStage::Start)?;
    let arguments: Vec<NamedTypeRow> = expanded_types(context, &header.arguments, |raw| {
        format!("{label} argument '{raw}' type")
    })?;
    raise_stage(context, &header, HeaderStage::Arguments)?;
    let (returns, return_columns): (String, Vec<NamedTypeRow>) = match &header.returns {
        Some(FunctionReturns::Table(columns)) => (
            TABLE_RETURNS.to_owned(),
            expanded_types(context, columns, |raw| {
                format!("{label} return column '{raw}' type")
            })?,
        ),
        Some(FunctionReturns::Type(text)) => (
            header_template(context, text, &format!("{label} returns"))?,
            Vec::new(),
        ),
        None => (String::from("None"), Vec::new()),
    };
    raise_stage(context, &header, HeaderStage::Returns)?;
    let namespace: FunctionNamespace =
        function_namespace(scope, &header_values, &label, FunctionLanguage::Sql)?;
    preserved(
        scope,
        &format!("SQL function '{name}'"),
        (&namespace.logical_database, &namespace.logical_schema),
    )?;
    let consumer: Bound<'_, PyAny> = context
        .classes
        .visibility
        .resource_identity
        .call1((&scope.function_kind, &name))?;
    let declarations: Bound<'_, PyAny> = scoped_declarations(context, &file_path, &consumer)?;
    let expansion: AuthoredExpansion<'_> =
        expanded_authored_sql(context, (&body, &file_path), &declarations)?;
    let sql: &str = &expansion.sql;
    if let IntrinsicCheck::Rejected(message) = intrinsic_free(
        sql,
        &scope.model_loop.reserved_markers,
        &format!("SQL function '{name}'"),
    ) {
        return Err(compile_error(context.classes, &message, None));
    }
    if scope.validate {
        validated_types(scope, &label, &arguments, (&returns, &return_columns))?;
        parse_checked(
            context,
            sql,
            &format!("SQL syntax error in function '{name}' ({file_path})"),
        )?;
        if !return_columns.is_empty() {
            checked_output_contract(sql, return_columns.len(), &relative_path, context)?;
        }
    }
    let contents: String = contents.unwrap_or_default();
    let references: Vec<ReferenceRow> = function_references(
        scope,
        (&file_path, &relative_path, &contents),
        sql,
        expansion.argument_references,
    )?;
    raise_stage(context, &header, HeaderStage::Metadata)?;
    Ok(AttachedFunction {
        relative_path,
        destination: destination(&name, &namespace, false),
        fingerprint_destination: destination(&name, &namespace, true),
        name,
        arguments,
        returns,
        return_columns,
        body: expansion.sql,
        references,
        python: None,
        tags: header.tags,
        description: header.description,
        host: FunctionHandles {
            function_file,
            usages: expansion.usages.unbind(),
        },
    })
}

/// One Python function, raising each header error where Python's attachment raises it.
fn python_function(scope: &FunctionScope<'_, '_>, row: FunctionRow) -> PyResult<AttachedFunction> {
    let context: &LoopContext<'_, '_> = scope.context;
    let py: Python<'_> = context.host.py();
    let FunctionRow {
        relative_path,
        name,
        function_file,
        header_values,
        body,
        ..
    } = row;
    let header_values: Bound<'_, PyDict> = header_values.into_bound(py);
    let label: String = format!("Python function {relative_path}");
    let header: FunctionHeader =
        parsed_header(&header_values, FunctionLanguage::Python, &relative_path);
    raise_stage(context, &header, HeaderStage::Start)?;
    let arguments: Vec<NamedTypeRow> = expanded_types(context, &header.arguments, |raw| {
        format!("{label} argument '{raw}' type")
    })?;
    raise_stage(context, &header, HeaderStage::Arguments)?;
    let returns: String = match &header.returns {
        Some(FunctionReturns::Type(text)) => {
            header_template(context, text, &format!("{label} returns"))?
        }
        _ => String::from("None"),
    };
    raise_stage(context, &header, HeaderStage::PythonValues)?;
    let namespace: FunctionNamespace =
        function_namespace(scope, &header_values, &label, FunctionLanguage::Python)?;
    preserved(
        scope,
        &format!("Python function '{name}'"),
        (
            &namespace.fingerprint_logical_database,
            &namespace.fingerprint_logical_schema,
        ),
    )?;
    if scope.validate {
        validated_types(scope, &label, &arguments, (&returns, &[]))?;
    }
    raise_stage(context, &header, HeaderStage::Metadata)?;
    Ok(AttachedFunction {
        relative_path,
        destination: destination(&name, &namespace, false),
        fingerprint_destination: destination(&name, &namespace, true),
        name,
        arguments,
        returns,
        return_columns: Vec::new(),
        body,
        references: Vec::new(),
        python: Some(PythonRuntime {
            runtime_version: header.runtime_version,
            entry_point: header.entry_point,
            packages: header.packages,
        }),
        tags: header.tags,
        description: header.description,
        host: FunctionHandles {
            function_file,
            usages: PyTuple::empty(py).unbind(),
        },
    })
}

/// `_raise_header_failure`: the header's error when it is raised at `stage`.
fn raise_stage(
    context: &LoopContext<'_, '_>,
    header: &FunctionHeader,
    stage: HeaderStage,
) -> PyResult<()> {
    match &header.failure {
        Some(failure) if failure.stage == stage => {
            Err(compile_error(context.classes, &failure.message, None))
        }
        _ => Ok(()),
    }
}

/// Each `(name, type)` with its type's templates expanded, in order.
fn expanded_types(
    context: &LoopContext<'_, '_>,
    types: &[NamedType],
    label: impl Fn(&str) -> String,
) -> PyResult<Vec<NamedTypeRow>> {
    types
        .iter()
        .map(|named| {
            Ok((
                named.name.clone(),
                header_template(context, &named.type_text, &label(&named.raw_name))?,
            ))
        })
        .collect()
}

/// `_expand_function_header_value`: templates expanded, `@@CTX` tokens kept.
fn header_template(context: &LoopContext<'_, '_>, raw: &str, label: &str) -> PyResult<String> {
    let py: Python<'_> = context.host.py();
    expanded_template(context, PyString::new(py, raw).as_any(), label, true)?
        .str()?
        .extract()
}

fn optional_template(
    context: &LoopContext<'_, '_>,
    raw: Option<String>,
    label: &str,
) -> PyResult<Option<String>> {
    raw.map(|raw| header_template(context, &raw, label))
        .transpose()
}

/// `_expand_function_environment_value`: a preserved target dimension is unset.
fn environment_value(
    context: &LoopContext<'_, '_>,
    raw: Option<&str>,
    label: &str,
) -> PyResult<Option<String>> {
    match raw {
        None | Some(PRESERVE_TARGET_VALUE) => Ok(None),
        Some(raw) => Ok(Some(header_template(context, raw, label)?)),
    }
}

/// The function's namespace from its own string `database` and `schema` header values.
fn function_namespace(
    scope: &FunctionScope<'_, '_>,
    header_values: &Bound<'_, PyDict>,
    label: &str,
    language: FunctionLanguage,
) -> PyResult<FunctionNamespace> {
    let mut header: [Option<String>; 2] = [None, None];
    for (slot, key) in header.iter_mut().zip([DATABASE_KEY, SCHEMA_KEY]) {
        if let Some(value) = header_values.get_item(key)?
            && let Ok(text) = value.downcast::<PyString>()
        {
            let text: String = text.to_string_lossy().into_owned();
            *slot = Some(header_template(
                scope.context,
                &text,
                &format!("{label} {key}"),
            )?);
        }
    }
    let [header_database, header_schema] = header;
    Ok(resolve_function_namespace(&NamespaceInputs {
        header_database,
        header_schema,
        default_database: scope.defaults.0.clone(),
        default_schema: scope.defaults.1.clone(),
        target_database: scope.targets.0.clone(),
        target_schema: scope.targets.1.clone(),
        language,
        inherit_default_namespace: scope.inherit,
    }))
}

/// `validate_preserved_logical_namespace`.
fn preserved(
    scope: &FunctionScope<'_, '_>,
    label: &str,
    logical: (&Option<String>, &Option<String>),
) -> PyResult<()> {
    match preserved_logical_namespace(
        label,
        scope.target.as_ref(),
        (logical.0.as_ref(), logical.1.as_ref()),
    ) {
        Ok(()) => Ok(()),
        Err(AssemblyError::CompileInput { message, help }) => {
            let py: Python<'_> = scope.context.host.py();
            let keywords: Bound<'_, PyDict> = PyDict::new(py);
            keywords.set_item("help", help)?;
            let error: Bound<'_, PyAny> = scope
                .context
                .classes
                .compile_error
                .call((message,), Some(&keywords))?;
            Err(PyErr::from_value(error))
        }
        Err(other) => Err(PyValueError::new_err(format!("{other:?}"))),
    }
}

/// `validate_native_type` for every argument, then the return columns or return type.
fn validated_types(
    scope: &FunctionScope<'_, '_>,
    label: &str,
    arguments: &[NamedTypeRow],
    returns: (&str, &[NamedTypeRow]),
) -> PyResult<()> {
    for (name, type_text) in arguments {
        validated_type(scope, type_text, &format!("{label} argument '{name}'"))?;
    }
    let (returns, return_columns) = returns;
    if return_columns.is_empty() {
        return validated_type(scope, returns, &format!("{label} return type"));
    }
    for (name, type_text) in return_columns {
        validated_type(scope, type_text, &format!("{label} return column '{name}'"))?;
    }
    Ok(())
}

fn validated_type(scope: &FunctionScope<'_, '_>, type_sql: &str, label: &str) -> PyResult<()> {
    match function_type_error(type_sql, &scope.adapter_name, label) {
        Ok(None) => Ok(()),
        Ok(Some(message)) => Err(compile_error(scope.context.classes, &message, None)),
        Err(message) => Err(PyValueError::new_err(message)),
    }
}

/// `_validate_table_function_output_contract`: the body projects as many columns as declared.
fn checked_output_contract(
    sql: &str,
    declared: usize,
    relative_path: &str,
    context: &LoopContext<'_, '_>,
) -> PyResult<()> {
    let request = QueryColumnsRequest {
        dialect: GENERIC_DIALECT.to_owned(),
        normalization_dialect: GENERIC_DIALECT.to_owned(),
        function_return_types: Vec::new(),
        nullability_rules: Some(Vec::new()),
        nullability_callback: None,
        types: Vec::new(),
        nullability: Vec::new(),
        queries: vec![QueryColumnsInput {
            sql: sql.to_owned(),
            placeholders: Vec::new(),
            analysis_names: Vec::new(),
            lineage_references: Vec::new(),
            recover_cte_facts: false,
            mode: QueryColumnsMode::Parse,
        }],
    };
    let columns: Vec<QueryColumns> = query_columns(None, &request).map_err(query_columns_error)?;
    let inferred: usize = columns
        .first()
        .and_then(|query| query.columns.as_ref())
        .map_or(0, Vec::len);
    if inferred == 0 || inferred == declared {
        return Ok(());
    }
    let message: String = format!(
        "SQL table function {relative_path} declares {declared} return columns but its query \
         produces {inferred}"
    );
    Err(compile_error(context.classes, &message, None))
}

/// The body's references, reporting rejected calls, then validated against the project.
fn function_references(
    scope: &FunctionScope<'_, '_>,
    file: (&str, &str, &str),
    sql: &str,
    argument_references: Vec<ReferenceRow>,
) -> PyResult<Vec<ReferenceRow>> {
    let (file_path, relative_path, contents) = file;
    let context: &LoopContext<'_, '_> = scope.context;
    let syntax = &scope.model_loop.syntax;
    let scanned = match extract_sql_references(sql, syntax) {
        ReferenceExtraction::Extracted(scanned) => scanned,
        ReferenceExtraction::Failed(failure) => {
            let origin: ScanFaultOrigin<'_> = ScanFaultOrigin {
                relative_path,
                contents,
                query_sql: sql,
                passes: None,
            };
            let message: String = located_scan_error(&failure, sql, &origin, syntax);
            return Err(compile_error(context.classes, &message, None));
        }
    };
    report_invalid_calls(
        context,
        syntax,
        &scanned.invalid_calls,
        CallOrigin {
            name: None,
            relative_path,
            file_path,
            contents,
        },
    )?;
    let references: Vec<ReferenceRow> = merge_call_site_references(
        scanned.references.iter().map(reference_row).collect(),
        argument_references,
    );
    let validator: Bound<'_, NativeModelValidator> = scope
        .model_loop
        .validator
        .bind(context.host.py())
        .downcast()?
        .clone();
    let known: KnownResources<'_> = validator.get().known_resources();
    let calls: Vec<(&str, &str)> = references
        .iter()
        .map(|ReferenceRow { kind, name, .. }| (kind.as_str(), name.as_str()))
        .collect();
    record_source_reads(
        calls
            .iter()
            .filter(|(kind, _)| *kind == SOURCE_KIND)
            .map(|(_, name)| *name),
    );
    if let Some(message) = function_reference_error(relative_path, &calls, &known) {
        return Err(compile_error(context.classes, &message, None));
    }
    Ok(references)
}

/// The physical, or fingerprint, destination of one function.
fn destination(name: &str, namespace: &FunctionNamespace, fingerprint: bool) -> DestinationFact {
    let (database, schema, logical_database, logical_schema) = if fingerprint {
        (
            &namespace.fingerprint_database,
            &namespace.fingerprint_schema,
            &namespace.fingerprint_logical_database,
            &namespace.fingerprint_logical_schema,
        )
    } else {
        (
            &namespace.database,
            &namespace.schema,
            &namespace.logical_database,
            &namespace.logical_schema,
        )
    };
    DestinationFact {
        database: database.clone(),
        schema: schema.clone(),
        name: name.to_owned(),
        qualified_name: None,
        logical_schema: logical_schema.clone(),
        logical_database: logical_database.clone(),
    }
}

/// `validate_table_function_call_arities` over each model's, then each function's, calls.
pub(crate) fn checked_table_function_arities(
    context: &LoopContext<'_, '_>,
    models: &[(String, Vec<ReferenceRow>)],
    functions: &[AttachedFunction],
) -> PyResult<()> {
    let counts: HashMap<String, usize> = functions
        .iter()
        .filter(|function| !function.return_columns.is_empty())
        .map(|function| (function.name.clone(), function.arguments.len()))
        .collect();
    let mut owners: Vec<(String, &[ReferenceRow])> = Vec::new();
    for (path, references) in models {
        owners.push((format!("Model file {path}"), references));
    }
    for function in functions {
        owners.push((
            format!("SQL function file {}", function.relative_path),
            &function.references,
        ));
    }
    for (owner, references) in owners {
        let calls: Vec<(&str, &str, Option<usize>)> = references
            .iter()
            .map(
                |ReferenceRow {
                     kind,
                     name,
                     call_argument_count: count,
                     ..
                 }| (kind.as_str(), name.as_str(), *count),
            )
            .collect();
        if let Some(message) = table_function_arity_error(&owner, &calls, &counts) {
            return Err(compile_error(context.classes, &message, None));
        }
    }
    Ok(())
}
