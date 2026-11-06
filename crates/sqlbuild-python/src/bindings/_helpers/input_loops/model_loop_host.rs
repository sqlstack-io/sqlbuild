//! The Python host the loop hands finished models and user-code steps to.

use std::sync::Arc;

use pyo3::FromPyObject;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_analysis::compiled_project::main::header_metadata_keys::is_header_metadata_key;
use sqlbuild_analysis::compiled_project::main::plain_model_facts::plain_model_facts;
use sqlbuild_analysis::compiled_project::models::{
    CompiledModelFacts, ModelConfigFacts, PlainModel,
};
use sqlbuild_analysis::sql_macros::models::TypedCall;
use sqlbuild_discovery::model_files::models::HeaderFacts;

use crate::bindings::_helpers::compiled_project::project::{
    CONTRACT_KEY, INCREMENTAL_MODE_KEY, materialization, text_value,
};
use crate::bindings::_helpers::compiled_project::project::{
    DeclarationPy, config_values, declarations,
};
use crate::bindings::_helpers::compiled_views::model_input_view::{
    InputProjections, ModelInputHandles, ModelInputRecord, SchemaHandles, SchemaRecord,
};
use crate::bindings::_helpers::discovery::model_file_record::{ModelFileTexts, NativeModelFile};
use crate::bindings::_helpers::input_loops::model_loop_header::AttachedHeader;
use crate::bindings::_helpers::input_loops::model_loop_inputs::{
    HOOK_KEYS, reference_fact, stripped_values,
};
use crate::bindings::_helpers::model_config::config_nodes::carried_node;
use crate::bindings::_helpers::model_config::typed_config::{retain_values, typed_values};
use crate::bindings::_helpers::records::fields::{field as native_field, replaced_keywords};
use crate::bindings::_helpers::render::declaration_expansion::{ExpandedDeclarations, span_rows};
use crate::bindings::_helpers::render::macro_call_rows::{SpanRow, span_row};
use crate::bindings::_helpers::render::macro_expansion::ExpansionOutput;
use crate::bindings::_helpers::sqltext::declaration_references::ScannedReference;
use sqlbuild_project::models::ExpansionRecord;
use sqlbuild_project::models::ReferenceRow;

const HEADER_KEYS_ATTRIBUTE: &str = "model_header_keys";
const LAYER_SCHEMA_ATTRIBUTE: &str = "layer_schema";
const VALUES_ARGUMENT: &str = "values";
const ENUMS_ARGUMENT: &str = "enums";
const CONSTANTS_ARGUMENT: &str = "constants";
const LOCAL_ENUMS_ATTRIBUTE: &str = "local_enums";
const LOCAL_CONSTANTS_ATTRIBUTE: &str = "local_constants";
const NAME_ATTRIBUTE: &str = "name";

/// A model's scanned declaration references and the scan's stop marker.
pub(crate) type DeclarationScan = (Vec<ScannedReference>, Option<u8>);

/// One model the loop builds: its discovered file and what the host prepared for it.
pub(crate) struct ModelLoopRow {
    pub(crate) paths: ModelFileTexts,
    pub(crate) authored_query: Arc<str>,
    pub(crate) contents: Arc<str>,
    pub(crate) scan: Option<DeclarationScan>,
    /// The `@@`-interpolated query, its template reads, its error, and its substitution spans.
    pub(crate) interpolation: InterpolationRow,
    /// The discovered model file, owned by the discovery inputs.
    pub(crate) model_file: Py<NativeModelFile>,
    /// What the pre-pass read from the model's native header, when it has one.
    pub(crate) header: Option<HeaderFacts>,
}

impl ModelLoopRow {
    pub(crate) fn new(
        file: (Py<NativeModelFile>, ModelFileTexts),
        scan: Option<DeclarationScan>,
        interpolation: InterpolationRow,
    ) -> Self {
        let (model_file, paths) = file;
        let record: &NativeModelFile = model_file.get();
        Self {
            paths,
            authored_query: record.query_text().clone(),
            contents: record.contents_text().clone(),
            scan,
            interpolation,
            model_file,
            header: None,
        }
    }
}
/// A model query's interpolation: the interpolated query, template reads, error and spans.
pub(crate) struct InterpolationRow {
    pub(crate) sql: String,
    pub(crate) reads: Vec<(String, String)>,
    pub(crate) error: Option<String>,
    /// The substitution spans.
    pub(crate) spans: Vec<SpanRow>,
}

/// One model the native loop finished, before the host builds its Python view.
pub(crate) struct FinishedModel<'py> {
    pub(crate) config: Bound<'py, PyAny>,
    /// The hook-expanded config values, header metadata still included.
    pub(crate) values: Bound<'py, PyDict>,
    pub(crate) query_sql: String,
    pub(crate) header: AttachedHeader<'py>,
    pub(crate) references: Vec<ReferenceRow>,
    pub(crate) declarations: ExpandedDeclarations<'py>,
    /// Whether the SQL is analysed, and where a rejected `sql_analysis false` was authored.
    pub(crate) analysis: (bool, Option<Bound<'py, PyAny>>),
    /// The usages of the model's named schema and Python hooks, recorded after its hook usages.
    pub(crate) named_usages: Vec<Bound<'py, PyAny>>,
    /// The query's macro expansion, when it calls macros.
    pub(crate) macros: Option<ExpansionOutput>,
    /// Each expression macro call in the query, by byte range.
    pub(crate) typed_calls: Vec<TypedCall>,
}

/// The Python classes a finished model's record is built with.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct RecordClasses<'py> {
    /// `declaration_rows`, the facts rows of the model's local enums and constants.
    declaration_rows: Bound<'py, PyAny>,
}

/// What the loop knows about a finished model besides its loop row.
pub(crate) struct RecordInputs<'a, 'py> {
    pub(crate) classes: &'a RecordClasses<'py>,
    /// The visible declarations, for the model's own enums and constants.
    pub(crate) visible: &'a Bound<'py, PyAny>,
}

/// A natively finished model's input record, and the facts the project retains.
pub(crate) fn finished_record<'py>(
    inputs: &RecordInputs<'_, 'py>,
    row: ModelLoopRow,
    finished: FinishedModel<'py>,
) -> PyResult<(ModelInputRecord, CompiledModelFacts)> {
    let py: Python<'py> = inputs.visible.py();
    let ModelLoopRow {
        paths:
            ModelFileTexts {
                name,
                posix_path: relative_path,
                ..
            },
        authored_query: authored_query_sql,
        contents: authored_sql,
        interpolation:
            InterpolationRow {
                sql: interpolated_sql,
                spans: interpolation_spans,
                ..
            },
        model_file,
        ..
    } = row;
    let stripped: Bound<'py, PyDict> = stripped_values(py, &finished.values)?;
    let mut usages: Vec<Bound<'py, PyAny>> = finished.declarations.usages;
    if let Some(schema) = &finished.header.schema {
        usages.extend(schema.generated_usages.iter().cloned());
    }
    let enum_columns: Vec<String> = match &finished.header.schema {
        Some(schema) => schema.enum_columns.keys().extract()?,
        None => Vec::new(),
    };
    let local_enums: Bound<'py, PyTuple> =
        dict_values(&native_field(inputs.visible, LOCAL_ENUMS_ATTRIBUTE)?)?;
    let local_constants: Bound<'py, PyTuple> =
        dict_values(&native_field(inputs.visible, LOCAL_CONSTANTS_ATTRIBUTE)?)?;
    let rows_arguments: Bound<'py, PyDict> = PyDict::new(py);
    rows_arguments.set_item(ENUMS_ARGUMENT, &local_enums)?;
    rows_arguments.set_item(CONSTANTS_ARGUMENT, &local_constants)?;
    let (enums, constants): (Vec<DeclarationPy<'py>>, Vec<DeclarationPy<'py>>) = inputs
        .classes
        .declaration_rows
        .call((), Some(&rows_arguments))?
        .extract()?;
    let replaced: Bound<'py, PyDict> = PyDict::new(py);
    replaced.set_item(VALUES_ARGUMENT, &stripped)?;
    let config: Bound<'py, PyAny> = replaced_keywords(&finished.config, &replaced)?;
    let (_, typed) = typed_values(&finished.config)?;
    if let Some(built) = &typed.get().built {
        let mut nodes = built.as_ref().clone();
        nodes.retain(|key, _| !is_header_metadata_key(key));
        for key in HOOK_KEYS {
            if let Some(value) = stripped.get_item(key)? {
                nodes.insert(key.to_owned(), Arc::new(carried_node(&value)?));
            }
        }
        retain_values(&config, Some(&Arc::new(nodes)))?;
    }
    let mut declaration_usages: Vec<Bound<'py, PyAny>> = unique(py, usages)?;
    declaration_usages.extend(finished.named_usages);
    let sql_macro_names: Vec<String> = finished.declarations.sql_macros.clone();
    let (macro_sql, macro_spans, mut macro_deps, macro_usages) = match finished.macros {
        Some(macros) => {
            let mut names: Vec<String> = Vec::new();
            for dependency in unique(
                py,
                macros
                    .dependencies
                    .into_iter()
                    .map(|item| item.into_bound(py))
                    .collect(),
            )? {
                names.push(native_field(&dependency, NAME_ATTRIBUTE)?.extract()?);
            }
            (
                macros.sql,
                macros
                    .spans
                    .into_iter()
                    .map(span_row)
                    .collect::<Vec<SpanRow>>(),
                names,
                unique(
                    py,
                    macros
                        .usages
                        .into_iter()
                        .map(|item| item.into_bound(py))
                        .collect(),
                )?,
            )
        }
        None => (
            finished.declarations.spliced.sql.clone(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        ),
    };
    for name in sql_macro_names {
        if !macro_deps.contains(&name) {
            macro_deps.push(name);
        }
    }
    let schema: Option<SchemaRecord> = finished.header.schema.as_ref().map(|schema| SchemaRecord {
        description: finished.header.description.clone(),
        type_enforcement: schema.type_enforcement,
        model_schema: schema.model_schema.clone(),
        host: SchemaHandles {
            columns: schema.columns.clone(),
            audits: schema.audits.clone(),
            dynamic_columns: schema.dynamic_columns.clone(),
        },
    });
    let authored: Arc<str> = authored_query_sql.clone();
    let macro_source: Arc<str> = shared_sql(&finished.declarations.spliced.sql, &[&authored]);
    let expanded: Arc<str> = shared_sql(&macro_sql, &[&authored, &macro_source]);
    let query: Arc<str> = shared_sql(&finished.query_sql, &[&authored, &macro_source, &expanded]);
    let record: ModelInputRecord = ModelInputRecord {
        name: name.clone(),
        query_sql: Arc::clone(&query),
        macro_source_sql: macro_source,
        references: finished.references.clone(),
        schema,
        sql_validation_enabled: finished.analysis.0,
        macro_deps,
        sql_macro_checks: finished.declarations.sql_macro_checks.clone(),
        typed_calls: finished.typed_calls.clone(),
        expansion: ExpansionRecord {
            authored: *interpolated_sql == *authored_query_sql,
            authored_sql: authored,
            expanded_sql: expanded,
            interpolation_spans,
            declaration_spans: span_rows(&finished.declarations.spliced),
            macro_spans,
        },
        compiled: None,
        host: ModelInputHandles {
            model_file,
            config: config.unbind(),
            rejected_opt_out: finished.analysis.1.map(Bound::unbind),
            enum_declarations: local_enums.unbind(),
            constant_declarations: local_constants.unbind(),
            enum_columns: finished
                .header
                .schema
                .as_ref()
                .map_or_else(|| PyDict::new(py), |schema| schema.enum_columns.clone())
                .unbind(),
            macro_usages: PyTuple::new(py, macro_usages)?.unbind(),
            declaration_usages: PyTuple::new(py, unique(py, declaration_usages)?)?.unbind(),
            projections: InputProjections::default(),
        },
    };
    let (values, encoding_error) = config_values(&stripped);
    let expansion_passes: Vec<Vec<SpanRow>> = if *record.query_sql == *record.expansion.expanded_sql
    {
        vec![
            record.expansion.interpolation_spans.clone(),
            record.expansion.declaration_spans.clone(),
            record.expansion.macro_spans.clone(),
        ]
    } else {
        Vec::new()
    };
    let facts: CompiledModelFacts = plain_model_facts(PlainModel {
        name,
        relative_path,
        query_sql: query,
        authored_query_sql,
        authored_sql,
        config: ModelConfigFacts {
            values,
            encoding_error,
            header_keys: native_field(&finished.config, HEADER_KEYS_ATTRIBUTE)?.extract()?,
            layer_schema: native_field(&finished.config, LAYER_SCHEMA_ATTRIBUTE)?.extract()?,
            contract: text_value(&stripped, CONTRACT_KEY)?,
            materialized: materialization(&stripped)?,
            incremental_mode: text_value(&stripped, INCREMENTAL_MODE_KEY)?,
        },
        schema: finished.header.schema.map(|schema| schema.fact),
        references: finished
            .references
            .into_iter()
            .map(reference_fact)
            .collect(),
        enum_columns,
        enum_declarations: declarations(enums)?,
        constant_declarations: declarations(constants)?,
        typed_calls: finished.typed_calls,
        expansion_passes,
    });
    Ok((record, facts))
}

/// Reuse an identical immutable spelling already owned by this model.
fn shared_sql(sql: &str, candidates: &[&Arc<str>]) -> Arc<str> {
    for candidate in candidates {
        if ***candidate == *sql {
            return Arc::clone(candidate);
        }
    }
    Arc::from(sql)
}

/// A dict's values in insertion order.
fn dict_values<'py>(mapping: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyTuple>> {
    let mapping: &Bound<'py, PyDict> = mapping.downcast()?;
    PyTuple::new(py_of(mapping), mapping.values())
}

fn py_of<'py>(mapping: &Bound<'py, PyDict>) -> Python<'py> {
    mapping.py()
}

/// `dict.fromkeys(items)`: each item once, first occurrence first, by Python equality.
fn unique<'py>(py: Python<'py>, items: Vec<Bound<'py, PyAny>>) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let seen: Bound<'py, PyDict> = PyDict::new(py);
    for item in items {
        seen.set_item(item, py.None())?;
    }
    Ok(seen.keys().into_iter().collect())
}
