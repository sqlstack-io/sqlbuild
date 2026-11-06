//! What each model, seed, source and table function declares about its columns before analysis.

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use std::collections::HashMap;

use pyo3::types::{PyDict, PyDictMethods, PyString};
use sqlbuild_analysis::assembly::analysis_session::types::{Pairs, Shapes};
use sqlbuild_analysis::compiled_project::models::SchemaColumnFact;

use crate::bindings::_helpers::compiled_project::schema_facts::python_column_facts;
use crate::bindings::_helpers::compiled_views::model_input_view::{
    attached_columns, attached_schema,
};
use crate::bindings::_helpers::compiled_views::source_views::{
    AttachedSource, SourceViews, read_source_view,
};
use crate::bindings::_helpers::records::construct::enum_member;
use crate::bindings::_helpers::records::fields::{enum_member_value, field as native_field};
use crate::bindings::_helpers::records::ids::RecordId;

const UNKNOWN_TYPE: &str = "UNKNOWN";
const NOT_NULL_AUDIT_NAME: &str = "not_null";
const ENFORCED_CONTRACT: &str = "enforced";
const CONTRACT_KEY: &str = "contract";
const TABLE_FUNCTION_PREFIX: &str = "__sqlbuild_table_function_";
const NON_NULL_MEMBER: &str = "NON_NULL";
const UNKNOWN_MEMBER: &str = "UNKNOWN";
/// `str(None)`, the text of an exact declaration without a type.
const NONE_TEXT: &str = "None";

/// The three per-relation declaration maps the analysis starts from, and the declared shapes.
pub(crate) struct DeclaredRelations<'py> {
    pub(crate) column_types: Bound<'py, PyDict>,
    pub(crate) nullability: Bound<'py, PyDict>,
    pub(crate) complete_schemas: Bound<'py, PyDict>,
    pub(crate) declared: DeclaredShapes,
}

/// Relation shapes as rows in a dict's insertion order, each value as `str(value)`.
#[derive(Debug, Clone, Default)]
pub(crate) struct ShapeRows {
    rows: Shapes,
    positions: HashMap<String, usize>,
}

impl ShapeRows {
    /// `shapes[name] = pairs`, keeping an existing name's position as a dict does.
    pub(crate) fn set(&mut self, name: String, pairs: Pairs) {
        match self.positions.get(&name) {
            Some(&position) => self.rows[position].1 = pairs,
            None => {
                let _ = self.positions.insert(name.clone(), self.rows.len());
                self.rows.push((name, pairs));
            }
        }
    }

    /// The rows in insertion order.
    pub(crate) fn rows(&self) -> &Shapes {
        &self.rows
    }
}

/// `(column, value)` pairs as a dict keeps them: each column once, at its first position, last value.
pub(crate) fn dict_pairs(items: impl IntoIterator<Item = (String, String)>) -> Pairs {
    let mut pairs: Pairs = Vec::new();
    let mut positions: HashMap<String, usize> = HashMap::new();
    for (column, value) in items {
        match positions.get(&column) {
            Some(&position) => pairs[position].1 = value,
            None => {
                let _ = positions.insert(column.clone(), pairs.len());
                pairs.push((column, value));
            }
        }
    }
    pairs
}

/// The native rows of the declared types, nullability and authoritative schemas.
#[derive(Debug, Clone, Default)]
pub(crate) struct DeclaredShapes {
    pub(crate) types: ShapeRows,
    pub(crate) nullability: ShapeRows,
    pub(crate) complete: ShapeRows,
}

/// The Python declaration maps being built: declared types, nullability and authoritative schemas.
type DeclaredMaps<'py> = (Bound<'py, PyDict>, Bound<'py, PyDict>, Bound<'py, PyDict>);

/// The native rows of the maps being built and the nullability texts they hold.
struct Declared {
    shapes: DeclaredShapes,
    non_null_text: &'static str,
    unknown_text: &'static str,
}

impl Declared {
    fn new() -> PyResult<Self> {
        let text = |member: &str| -> PyResult<&'static str> {
            enum_member_value(RecordId::InferredNullability.name(), member).ok_or_else(|| {
                PyRuntimeError::new_err(format!("InferredNullability has no member {member}"))
            })
        };
        Ok(Self {
            shapes: DeclaredShapes::default(),
            non_null_text: text(NON_NULL_MEMBER)?,
            unknown_text: text(UNKNOWN_MEMBER)?,
        })
    }

    /// Record `name`'s declared types and declared nullability.
    fn declare<'py>(
        &mut self,
        maps: &DeclaredMaps<'py>,
        name: &Bound<'py, PyAny>,
        columns: &[SchemaColumnFact],
    ) -> PyResult<()> {
        let py: Python<'py> = name.py();
        let key: String = name.extract()?;
        maps.0.set_item(name, column_types(py, columns, false)?)?;
        self.shapes
            .types
            .set(key.clone(), type_pairs(columns, false));
        maps.1.set_item(name, column_nullability(py, columns)?)?;
        let pairs: Pairs = dict_pairs(columns.iter().map(|column| {
            let text: &str = if non_null(column) {
                self.non_null_text
            } else {
                self.unknown_text
            };
            (column.name.clone(), text.to_owned())
        }));
        self.shapes.nullability.set(key, pairs);
        Ok(())
    }

    /// Record `name`'s declared types as an authoritative schema.
    fn complete<'py>(
        &mut self,
        maps: &DeclaredMaps<'py>,
        name: &Bound<'py, PyAny>,
        (columns, exact): (&[SchemaColumnFact], bool),
    ) -> PyResult<()> {
        maps.2
            .set_item(name, column_types(name.py(), columns, exact)?)?;
        self.shapes
            .complete
            .set(name.extract()?, type_pairs(columns, exact));
        Ok(())
    }
}

/// Each column's declared type text as `str` reads the declared types map.
fn type_pairs(columns: &[SchemaColumnFact], exact: bool) -> Pairs {
    dict_pairs(columns.iter().map(|column| {
        let text: &str = match column.data_type.as_deref() {
            Some(declared) if exact || !declared.is_empty() => declared,
            None if exact => NONE_TEXT,
            _ => UNKNOWN_TYPE,
        };
        (column.name.clone(), text.to_owned())
    }))
}

/// Whether a column is declared non-null: `nullable` false or a `not_null` audit.
fn non_null(column: &SchemaColumnFact) -> bool {
    column.nullable == Some(false)
        || column
            .audit_definitions
            .iter()
            .any(|definition| definition == NOT_NULL_AUDIT_NAME)
}

/// `{column.name: column.type or "UNKNOWN"}`, or the declared type unchanged when `exact`.
fn column_types<'py>(
    py: Python<'py>,
    columns: &[SchemaColumnFact],
    exact: bool,
) -> PyResult<Bound<'py, PyDict>> {
    let types: Bound<'py, PyDict> = PyDict::new(py);
    for column in columns {
        match column.data_type.as_deref() {
            Some(declared) if exact || !declared.is_empty() => {
                types.set_item(&column.name, declared)?;
            }
            None if exact => types.set_item(&column.name, py.None())?,
            _ => types.set_item(&column.name, UNKNOWN_TYPE)?,
        }
    }
    Ok(types)
}

/// Each column's declared nullability: non-null when `nullable` is false or a `not_null` audit.
fn column_nullability<'py>(
    py: Python<'py>,
    columns: &[SchemaColumnFact],
) -> PyResult<Bound<'py, PyDict>> {
    let facts: Bound<'py, PyDict> = PyDict::new(py);
    let non_null_member: Bound<'py, PyAny> =
        enum_member(py, RecordId::InferredNullability, NON_NULL_MEMBER)?;
    let unknown_member: Bound<'py, PyAny> =
        enum_member(py, RecordId::InferredNullability, UNKNOWN_MEMBER)?;
    for column in columns {
        let state: &Bound<'py, PyAny> = if non_null(column) {
            &non_null_member
        } else {
            &unknown_member
        };
        facts.set_item(&column.name, state)?;
    }
    Ok(facts)
}

fn table_function_name(function: &Bound<'_, PyAny>) -> PyResult<String> {
    let name: String = native_field(function, "name")?.extract()?;
    Ok(format!("{TABLE_FUNCTION_PREFIX}{name}"))
}

/// Record one source's declared types and nullability, and its schema when its contract is enforced.
fn declare_source<'py>(
    (declared, maps): (&mut Declared, &DeclaredMaps<'py>),
    (name, columns, enforced): (&Bound<'py, PyAny>, &[SchemaColumnFact], bool),
) -> PyResult<()> {
    declared.declare(maps, name, columns)?;
    if enforced && !columns.is_empty() {
        declared.complete(maps, name, (columns, false))?;
    }
    Ok(())
}

/// Declared column types, declared nullability and the authoritative binding schemas.
pub(crate) fn declared_relations<'py>(
    inputs: Bound<'py, PyAny>,
) -> PyResult<DeclaredRelations<'py>> {
    let py: Python<'py> = inputs.py();
    let maps: DeclaredMaps<'py> = (PyDict::new(py), PyDict::new(py), PyDict::new(py));
    let mut declared: Declared = Declared::new()?;
    for model in native_field(&inputs, "model_inputs")?.try_iter()? {
        let model: Bound<'py, PyAny> = model?;
        let Some(schema) = attached_schema(&model)? else {
            continue;
        };
        let (name, columns): (Bound<'py, PyAny>, Vec<SchemaColumnFact>) = (
            PyString::new(py, &schema.name).into_any(),
            attached_columns(&model)?,
        );
        declared.declare(&maps, &name, &columns)?;
        let contract: Option<Bound<'py, PyAny>> =
            native_field(&native_field(&model, "config")?, "values")?
                .downcast_into::<PyDict>()?
                .get_item(CONTRACT_KEY)?;
        let enforced: bool = match contract {
            Some(contract) => contract.eq(ENFORCED_CONTRACT)?,
            None => false,
        };
        if enforced && !columns.is_empty() && schema.dynamic_columns.is_empty() {
            declared.complete(&maps, &name, (&columns, false))?;
        }
    }
    let mut seeds: Vec<(Bound<'py, PyAny>, Vec<SchemaColumnFact>)> = Vec::new();
    for seed in native_field(&inputs, "seed_inputs")?.try_iter()? {
        let entry: Bound<'py, PyAny> = native_field(&seed?, "schema_entry")?;
        let name: Bound<'py, PyAny> = native_field(&entry, "name")?;
        let columns: Vec<SchemaColumnFact> =
            python_column_facts(&native_field(&entry, "columns")?, None)?;
        declared.declare(&maps, &name, &columns)?;
        seeds.push((name, columns));
    }
    for source in native_field(&inputs, "source_inputs")?.try_iter()? {
        let source: Bound<'py, PyAny> = source?;
        let native = read_source_view(&source, |record, py| {
            let attached: &AttachedSource = record.attached();
            let name: Bound<'py, PyAny> = PyString::new(py, &attached.name).into_any();
            let enforced: bool = attached.contract.as_deref() == Some(ENFORCED_CONTRACT);
            declare_source(
                (&mut declared, &maps),
                (&name, &record.key_facts(py)?.columns, enforced),
            )
        });
        if let Some(outcome) = native {
            outcome?;
            continue;
        }
        let entry: Bound<'py, PyAny> = native_field(&source, "source_entry")?;
        let name: Bound<'py, PyAny> = native_field(&entry, "name")?;
        let columns: Vec<SchemaColumnFact> =
            python_column_facts(&native_field(&entry, "columns")?, None)?;
        let enforced: bool = entry.getattr(CONTRACT_KEY)?.eq(ENFORCED_CONTRACT)?;
        declare_source((&mut declared, &maps), (&name, &columns, enforced))?;
    }
    for (name, columns) in seeds {
        if !columns.is_empty() {
            declared.complete(&maps, &name, (&columns, false))?;
        }
    }
    for function in native_field(&inputs, "sql_function_inputs")?.try_iter()? {
        let function: Bound<'py, PyAny> = function?;
        let columns: Vec<SchemaColumnFact> =
            python_column_facts(&native_field(&function, "return_columns")?, None)?;
        if columns.is_empty() {
            continue;
        }
        let name: Bound<'py, PyAny> =
            PyString::new(py, &table_function_name(&function)?).into_any();
        maps.0.set_item(&name, column_types(py, &columns, true)?)?;
        declared
            .shapes
            .types
            .set(name.extract()?, type_pairs(&columns, true));
        let unknown: Bound<'py, PyDict> = PyDict::new(py);
        for column in &columns {
            unknown.set_item(
                &column.name,
                enum_member(py, RecordId::InferredNullability, UNKNOWN_MEMBER)?,
            )?;
        }
        maps.1.set_item(&name, unknown)?;
        let unknown_text: &str = declared.unknown_text;
        declared.shapes.nullability.set(
            name.extract()?,
            dict_pairs(
                columns
                    .iter()
                    .map(|column| (column.name.clone(), unknown_text.to_owned())),
            ),
        );
        declared.complete(&maps, &name, (&columns, true))?;
    }
    let (types, nulls, complete) = maps;
    Ok(DeclaredRelations {
        column_types: types,
        nullability: nulls,
        complete_schemas: complete,
        declared: declared.shapes,
    })
}
