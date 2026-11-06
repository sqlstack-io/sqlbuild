//! A model's MODEL header attached as its schema entry, by the native header parser.

use std::sync::Arc;

use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyStringMethods};
use sqlbuild_analysis::compiled_project::models::{
    DynamicFamilyFact, ModelSchemaFact, SchemaColumnFact,
};
use sqlbuild_discovery::models::LineColumnSpan;
use sqlbuild_model_config::config_values::models::ConfigNode;
use sqlbuild_model_config::header_metadata::main::audit_factory_references::audit_factory_references;
use sqlbuild_model_config::header_metadata::main::merged_audit_identities::merged_audit_identities;
use sqlbuild_model_config::header_metadata::main::parse_dynamic_columns::parse_dynamic_columns;
use sqlbuild_model_config::header_metadata::main::parse_header_metadata::parse_header_metadata;
use sqlbuild_model_config::header_metadata::models::ParsedColumn;
use sqlbuild_model_config::model_validation::models::ColumnMigration;
use sqlbuild_model_config::types::NodeKind;

use crate::bindings::_helpers::compile_driver::stage_work::{
    NATIVE_HEADER_COLUMNS, PARSED_HEADER_COLUMNS, count_stage,
};
use crate::bindings::_helpers::compiled_project::schema_facts::family_fact;
use crate::bindings::_helpers::discovery::model_file_record::NativeModelFile;
use crate::bindings::_helpers::input_loops::model_loop::{
    LoopContext, compile_error, config_error,
};
use crate::bindings::_helpers::input_loops::model_loop_contracts::{
    EnumColumns, enum_columns, factory_audits,
};
use crate::bindings::_helpers::input_loops::model_loop_inputs::present;
use crate::bindings::_helpers::input_loops::model_loop_schema::{
    DeclaredColumns, ModelSchema, merged_columns, schema_enums,
};
use crate::bindings::_helpers::model_config::column_migrations::column_migration_error;
use crate::bindings::_helpers::model_config::config_errors::native_config_error;
use crate::bindings::_helpers::model_config::header_audits::HeaderAudits;
use crate::bindings::_helpers::model_config::header_families::HeaderFamilies;
use crate::bindings::_helpers::model_config::header_metadata::{
    HeaderColumn, HeaderColumns, HeaderValue, NativeColumnSpec, native_schema_column,
    parsed_audit_identity,
};
use crate::bindings::_helpers::model_config::typed_config::{config_node_of, typed_values};
use crate::bindings::_helpers::records::fields::field as native_field;
use sqlbuild_discovery::model_files::models::{HeaderFacts, NativeColumn};

const DESCRIPTION_KEY: &str = "description";
const COLUMNS_KEY: &str = "columns";
const AUDITS_KEY: &str = "audits";
const DYNAMIC_COLUMNS_KEY: &str = "dynamic_columns";
const AUDIT_FACTORIES_KEY: &str = "audit_factories";
const STRIP_METHOD: &str = "strip";
const ENUMS_FIELD: &str = "enums";

/// What a model's header schema entry is attached from.
pub(crate) struct HeaderSource<'a, 'py> {
    pub(crate) name: &'a str,
    pub(crate) values: &'a Bound<'py, PyDict>,
    pub(crate) config: &'a Bound<'py, PyAny>,
    pub(crate) model_file: &'a NativeModelFile,
    pub(crate) relative_path: &'a str,
    /// The model's visible declarations and the identity its usages are recorded for.
    pub(crate) visible: (&'a Bound<'py, PyAny>, &'a Bound<'py, PyAny>),
    pub(crate) schema: Option<&'a ModelSchema<'py>>,
    /// What the pre-pass read from the model's native header.
    pub(crate) facts: Option<&'a HeaderFacts>,
    pub(crate) model_file_handle: &'a Py<NativeModelFile>,
}

/// The header's description and, when it declares any schema metadata, its entry.
pub(crate) struct AttachedHeader<'py> {
    pub(crate) description: Option<String>,
    pub(crate) schema: Option<HeaderSchema<'py>>,
}

/// A header schema entry's Python contract objects, enum columns and retained fact.
pub(crate) struct HeaderSchema<'py> {
    pub(crate) columns: Arc<HeaderColumns>,
    pub(crate) audits: Arc<HeaderAudits>,
    pub(crate) dynamic_columns: Arc<HeaderFamilies>,
    pub(crate) type_enforcement: bool,
    pub(crate) enum_columns: Bound<'py, PyDict>,
    pub(crate) generated_usages: Vec<Bound<'py, PyAny>>,
    pub(crate) model_schema: Option<String>,
    pub(crate) fact: ModelSchemaFact,
}

/// The attached header entry, raising the attachment's errors in its order.
pub(crate) fn header_schema<'py>(
    py: Python<'py>,
    context: &LoopContext<'_, 'py>,
    source: &HeaderSource<'_, 'py>,
) -> PyResult<AttachedHeader<'py>> {
    let description: Option<String> = match (description(context, source)?, source.schema) {
        (None, Some(schema)) => native_field(&schema.declaration, DESCRIPTION_KEY)?.extract()?,
        (description, _) => description,
    };
    let columns: Option<Bound<'py, PyAny>> = present(source.values, COLUMNS_KEY)?;
    let audits: Option<Bound<'py, PyAny>> = present(source.values, AUDITS_KEY)?;
    let dynamic: Option<Bound<'py, PyAny>> = present(source.values, DYNAMIC_COLUMNS_KEY)?;
    let factories: Option<Bound<'py, PyAny>> = present(source.values, AUDIT_FACTORIES_KEY)?;
    if source.schema.is_none()
        && description.is_none()
        && columns.is_none()
        && audits.is_none()
        && dynamic.is_none()
        && factories.is_none()
    {
        return Ok(AttachedHeader {
            description,
            schema: None,
        });
    }
    let typed = typed_values(source.config)?;
    let model: (&str, &str) = (source.name, source.relative_path);
    let native: Option<&[NativeColumn]> = native_columns(context, source, columns.as_ref())?;
    if columns.is_some() {
        let stage = match native {
            Some(_) => NATIVE_HEADER_COLUMNS,
            None => PARSED_HEADER_COLUMNS,
        };
        count_stage(py, stage, 1)?;
    }
    let columns_node = if native.is_some() {
        Arc::new(ConfigNode::Null)
    } else {
        config_node_of(&typed, COLUMNS_KEY)?
    };
    let audits_node = config_node_of(&typed, AUDITS_KEY)?;
    let metadata = parse_header_metadata(
        &columns_node.as_ref(),
        &audits_node.as_ref(),
        source.relative_path,
    );
    let parsed_columns = metadata
        .columns
        .map_err(|error| raised_config_error(py, error))?;
    let model_path: &Bound<'py, PyAny> = &source.model_file.path_objects(py)?.1;
    let local_columns: DeclaredColumns = match native {
        Some(native) => native_declared_columns(context, source, native)?,
        None => {
            let locations: Bound<'py, PyDict> = source.model_file.header_locations(py)?;
            DeclaredColumns {
                specs: parsed_columns
                    .iter()
                    .map(|column| native_schema_column(py, context.contracts, column, &locations))
                    .collect::<PyResult<_>>()?,
                facts: local_column_facts(py, &parsed_columns)?,
            }
        }
    };
    let columns: DeclaredColumns = match source.schema {
        Some(schema) => merged_columns(
            context,
            schema,
            local_columns,
            (source.name, source.relative_path, model_path),
        )?,
        None => local_columns,
    };
    let dynamic_node = config_node_of(&typed, DYNAMIC_COLUMNS_KEY)?;
    let families = parse_dynamic_columns(&dynamic_node.as_ref(), model)
        .map_err(|error| raised_config_error(py, error))?;
    let parsed_audits = metadata
        .audits
        .map_err(|error| raised_config_error(py, error))?;
    let references: Vec<String> = match factories {
        Some(_) => {
            let available: Vec<String> = context.classes.audit_factories.keys().extract()?;
            let factories = config_node_of(&typed, AUDIT_FACTORIES_KEY)?;
            audit_factory_references(&factories.as_ref(), model, &available)
                .map_err(|error| raised_config_error(py, error))?
        }
        None => Vec::new(),
    };
    let mut audits = HeaderAudits::new(
        py,
        context.contracts,
        &parsed_audits,
        &py.None().into_bound(py),
    )?;
    let (generated, generated_identities) = factory_audits(py, context, &references)?;
    let direct_identities: Vec<String> = parsed_audits
        .iter()
        .map(|audit| parsed_audit_identity(py, audit))
        .collect::<PyResult<_>>()?;
    merged_audit_identities(&direct_identities, &generated_identities, model)
        .map_err(|error| raised_config_error(py, error))?;
    let migrations: Vec<ColumnMigration> = columns
        .facts
        .iter()
        .map(|column| ColumnMigration {
            name: column.name.clone(),
            migrate_from: column.migrate_from.clone(),
        })
        .collect();
    if let Some(error) = column_migration_error(source.name, source.values, &migrations)? {
        let error = native_config_error(py, error)?.into_bound(py).into_any();
        return Err(config_error(&error, Some(source.values)));
    }
    let type_enforcement: bool = !families.is_empty()
        || columns
            .facts
            .iter()
            .any(|column| column.data_type.is_some());
    let family_facts: Vec<DynamicFamilyFact> = families.iter().map(family_fact).collect();
    audits.extend(generated);
    let audits = Arc::new(audits);
    let families = Arc::new(HeaderFamilies::new(
        &context.contracts.schema_dynamic_column_family,
        families,
    )?);
    let enums: Bound<'py, PyDict> = native_field(source.visible.0, ENUMS_FIELD)?.downcast_into()?;
    let enums: Bound<'py, PyDict> = match source.schema {
        Some(schema) => {
            let merged: Bound<'py, PyDict> = enums.copy()?;
            merged.update(schema_enums(context, schema)?.as_mapping())?;
            merged
        }
        None => enums,
    };
    let resolved: EnumColumns<'py> =
        enum_columns(context, (&enums, source.visible), source.values, columns)?;
    let fact: ModelSchemaFact = ModelSchemaFact {
        audit_count: u32::try_from(audits.len()).unwrap_or(u32::MAX),
        columns: resolved.facts,
        dynamic_columns: family_facts,
        type_enforcement,
        named_schema: source.schema.is_some(),
    };
    Ok(AttachedHeader {
        description,
        schema: Some(HeaderSchema {
            columns: resolved.columns,
            audits,
            dynamic_columns: families,
            type_enforcement,
            enum_columns: resolved.declarations,
            generated_usages: resolved.usages,
            model_schema: source.schema.map(|schema| schema.name.clone()),
            fact,
        }),
    })
}

/// The natively parsed header columns, when the config's columns are the header's own value.
fn native_columns<'a>(
    context: &LoopContext<'_, '_>,
    source: &HeaderSource<'a, '_>,
    columns: Option<&Bound<'_, PyAny>>,
) -> PyResult<Option<&'a [NativeColumn]>> {
    let (Some(facts), Some(columns)) = (source.facts, columns) else {
        return Ok(None);
    };
    if !context.contracts.uses_default_classes()? {
        return Ok(None);
    }
    let Some(parsed) = &facts.columns else {
        return Ok(None);
    };
    let header: Bound<'_, PyDict> = source.model_file.header_value_dict(columns.py())?;
    let own: bool = match header.get_item(COLUMNS_KEY)? {
        Some(value) => value.is(columns) || value.eq(columns)?,
        None => false,
    };
    Ok(own.then_some(parsed.as_slice()))
}

/// Native header columns as declared columns, their records built only when Python reads them.
fn native_declared_columns(
    context: &LoopContext<'_, '_>,
    source: &HeaderSource<'_, '_>,
    columns: &[NativeColumn],
) -> PyResult<DeclaredColumns> {
    let py: Python<'_> = context.host.py();
    let rows: &[(String, LineColumnSpan)] = source.model_file.header_location_rows();
    let mut specs: Vec<HeaderColumn> = Vec::with_capacity(columns.len());
    let mut facts: Vec<SchemaColumnFact> = Vec::with_capacity(columns.len());
    for column in columns {
        let location: Option<LineColumnSpan> = rows
            .iter()
            .rev()
            .find(|(name, _)| *name == column.name)
            .map(|(_, span)| *span);
        facts.push(SchemaColumnFact {
            name: column.name.clone(),
            data_type: column.column_type.clone(),
            nullable: column.nullable,
            described: column.description.is_some(),
            migrate_from: column.migrate_from.clone(),
            audit_definitions: column
                .audits
                .iter()
                .map(|audit| audit.definition_name.clone())
                .collect(),
            location_path: None,
        });
        specs.push(HeaderColumn::native(
            &context.contracts.schema_column,
            NativeColumnSpec {
                column: column.clone(),
                location: location.map(|span| (source.model_file_handle.clone_ref(py), span)),
            },
        )?);
    }
    Ok(DeclaredColumns { specs, facts })
}

/// The header description: absent, or a non-blank string by Python's `str.strip`.
fn description(
    context: &LoopContext<'_, '_>,
    source: &HeaderSource<'_, '_>,
) -> PyResult<Option<String>> {
    let Some(value) = present(source.values, DESCRIPTION_KEY)? else {
        return Ok(None);
    };
    if let Ok(text) = value.downcast::<PyString>()
        && text.call_method0(STRIP_METHOD)?.is_truthy()?
    {
        return Ok(Some(text.to_str()?.to_owned()));
    }
    let message: String = format!(
        "{} model '{DESCRIPTION_KEY}' must be a non-empty string",
        source.relative_path
    );
    Err(compile_error(context.classes, &message, None))
}

fn raised_config_error(
    py: Python<'_>,
    error: sqlbuild_model_config::errors::ConfigError,
) -> pyo3::PyErr {
    match native_config_error(py, error) {
        Ok(native) => config_error(native.bind(py).as_any(), None),
        Err(failure) => failure,
    }
}

/// The facts of the header's own columns, which are declared in the model file.
fn local_column_facts<N: HeaderValue>(
    py: Python<'_>,
    parsed: &[ParsedColumn<N>],
) -> PyResult<Vec<SchemaColumnFact>> {
    let mut facts: Vec<SchemaColumnFact> = Vec::with_capacity(parsed.len());
    for column in parsed {
        let text = |node: Option<&N>| -> PyResult<Option<String>> {
            node.map(|node| node.exact_text(py)).transpose()
        };
        facts.push(SchemaColumnFact {
            name: column.name.exact_text(py)?,
            data_type: text(column.column_type.as_ref())?,
            nullable: column
                .nullable
                .as_ref()
                .map(|node| node.kind() == NodeKind::Bool(true)),
            described: column.description.is_some(),
            migrate_from: text(column.migrate_from.as_ref())?,
            audit_definitions: column
                .audits
                .iter()
                .map(|audit| audit.definition_name.exact_text(py))
                .collect::<PyResult<_>>()?,
            location_path: None,
        });
    }
    Ok(facts)
}
