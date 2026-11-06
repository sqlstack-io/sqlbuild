//! Header contract objects the model loop builds: factory audits, dynamic families, enum columns.

use std::sync::Arc;

use pyo3::BoundObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_analysis::compiled_project::models::SchemaColumnFact;

use crate::bindings::_helpers::input_loops::model_loop::LoopContext;
use crate::bindings::_helpers::input_loops::model_loop_schema::DeclaredColumns;
use crate::bindings::_helpers::model_config::config_nodes::{PyConfigNode, config_node};
use crate::bindings::_helpers::model_config::header_metadata::{HeaderColumn, HeaderColumns};
use crate::bindings::_helpers::records::fields::field as native_field;
use sqlbuild_model_config::config_values::models::ConfigNode;
use sqlbuild_model_config::header_metadata::models::ParsedAudit;

const CASES_ATTRIBUTE: &str = "cases";
const RELATIVE_PATH_ATTRIBUTE: &str = "relative_path";
const LINE_ATTRIBUTE: &str = "line";
const DEFINITION_ATTRIBUTE: &str = "definition";
const DEFINITION_NAME_FIELD: &str = "definition_name";
const ARGUMENTS_FIELD: &str = "arguments";
const NAME_FIELD: &str = "name";
const AUDITS_FIELD: &str = "audits";
const MEMBERS_FIELD: &str = "members";
const VALUE_FIELD: &str = "value";
const VALUES_ARGUMENT: &str = "values";
const SCALAR_TYPE_ATTRIBUTE: &str = "scalar_type";
const ENUM_VISIBILITY_ATTRIBUTE: &str = "enum_visibility";
const DECLARATION_FIELD: &str = "declaration";
const THROUGH_FIELD: &str = "through";
const CONSUMER_FIELD: &str = "consumer";
const KIND_FIELD: &str = "kind";
const PATH_FIELD: &str = "path";
const COLUMN_FIELD: &str = "column";
const LOCATION_FIELD: &str = "location";
const ACCEPTED_VALUES_AUDIT: &str = "accepted_values";
const CONTRACT_KEY: &str = "contract";
/// `AuditCase` fields copied onto each generated `SchemaAuditInstance` under the same name.
const COPIED_CASE_FIELDS: [&str; 9] = [
    "name",
    "severity",
    "run_scope",
    "always_run",
    "description",
    "thresholds",
    "minimum_samples",
    "evidence_limit",
    "meta",
];

/// A model's enum-typed columns resolved to scalar types, and the usages they generate.
pub(crate) struct EnumColumns<'py> {
    pub(crate) columns: Arc<HeaderColumns>,
    pub(crate) facts: Vec<SchemaColumnFact>,
    /// Column name to its enum declaration, as `CompileModelInput.enum_columns` holds them.
    pub(crate) declarations: Bound<'py, PyDict>,
    pub(crate) usages: Vec<Bound<'py, PyAny>>,
}

/// Each referenced factory's cases as audit instances, with their identities.
pub(crate) fn factory_audits<'py>(
    py: Python<'py>,
    context: &LoopContext<'_, 'py>,
    references: &[String],
) -> PyResult<(Vec<Bound<'py, PyAny>>, Vec<String>)> {
    let mut audits: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut identities: Vec<String> = Vec::new();
    for reference in references {
        let Some(factory) = context.classes.audit_factories.get_item(reference)? else {
            continue;
        };
        let location: Bound<'py, PyAny> = keywords(
            &context.contracts.source_location,
            &[
                (PATH_FIELD, native_field(&factory, RELATIVE_PATH_ATTRIBUTE)?),
                (LINE_ATTRIBUTE, native_field(&factory, LINE_ATTRIBUTE)?),
                (COLUMN_FIELD, 1_u8.into_pyobject_any(py)?),
            ],
        )?;
        for case in native_field(&factory, CASES_ATTRIBUTE)?.try_iter()? {
            let case: Bound<'py, PyAny> = case?;
            let fields: Bound<'py, PyDict> = PyDict::new(py);
            fields.set_item(
                DEFINITION_NAME_FIELD,
                native_field(&case, DEFINITION_ATTRIBUTE)?,
            )?;
            fields.set_item(
                ARGUMENTS_FIELD,
                py.get_type::<PyDict>()
                    .call1((native_field(&case, ARGUMENTS_FIELD)?,))?,
            )?;
            for field in COPIED_CASE_FIELDS {
                fields.set_item(field, native_field(&case, field)?)?;
            }
            fields.set_item(LOCATION_FIELD, &location)?;
            let audit: Bound<'py, PyAny> = context
                .contracts
                .schema_audit_instance
                .call((), Some(&fields))?;
            identities.push(audit_identity(&audit)?);
            audits.push(audit);
        }
    }
    Ok((audits, identities))
}

/// `audit.name or audit.definition_name`.
fn audit_identity(audit: &Bound<'_, PyAny>) -> PyResult<String> {
    let name: Bound<'_, PyAny> = native_field(audit, NAME_FIELD)?;
    if name.is_truthy()? {
        return name.extract();
    }
    native_field(audit, DEFINITION_NAME_FIELD)?.extract()
}

/// `resolve_enum_contract_columns` and `_generated_enum_usages` for a header schema entry.
pub(crate) fn enum_columns<'py>(
    context: &LoopContext<'_, 'py>,
    visible: (
        &Bound<'py, PyDict>,
        (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    ),
    values: &Bound<'py, PyDict>,
    columns: DeclaredColumns,
) -> PyResult<EnumColumns<'py>> {
    let py: Python<'py> = values.py();
    let (enums, (declarations, consumer)) = visible;
    if enums.is_empty() {
        return Ok(EnumColumns {
            columns: Arc::new(HeaderColumns::new(columns.specs)),
            facts: columns.facts,
            declarations: PyDict::new(py),
            usages: Vec::new(),
        });
    }
    let enforced: bool = match values.get_item(CONTRACT_KEY)? {
        Some(value) => value.eq(&context.classes.contract_enforced)?,
        None => false,
    };
    let resolved: Bound<'py, PyDict> = PyDict::new(py);
    let mut resolved_columns: Vec<HeaderColumn> = Vec::with_capacity(columns.specs.len());
    let mut facts: Vec<SchemaColumnFact> = Vec::with_capacity(columns.facts.len());
    for (column, mut fact) in columns.specs.into_iter().zip(columns.facts) {
        let key: &str = fact.data_type.as_deref().unwrap_or_default();
        let Some(declaration) = enums.get_item(key)? else {
            resolved_columns.push(column);
            facts.push(fact);
            continue;
        };
        resolved.set_item(&fact.name, &declaration)?;
        if column.retains_audits() && context.contracts.uses_default_classes()? {
            let mut generated = None;
            if enforced {
                let members = native_field(&declaration, MEMBERS_FIELD)?
                    .try_iter()?
                    .map(|member| config_node(&native_field(&member?, VALUE_FIELD)?))
                    .collect::<PyResult<Vec<PyConfigNode>>>()?;
                let mut present = false;
                for audit in column.audit_instances(py)? {
                    if audit.accepts_enum(&members)? {
                        present = true;
                        break;
                    }
                }
                if !present {
                    generated = Some(ParsedAudit {
                        definition_name: ConfigNode::Str(ACCEPTED_VALUES_AUDIT.to_owned()),
                        arguments: vec![(
                            ConfigNode::Str(VALUES_ARGUMENT.to_owned()),
                            ConfigNode::Tuple(members),
                        )],
                        name: None,
                        description: None,
                        severity: None,
                        run_scope: None,
                        always_run: None,
                        thresholds: None,
                        minimum_samples: None,
                        evidence_limit: None,
                        meta: None,
                    });
                    fact.audit_definitions
                        .push(ACCEPTED_VALUES_AUDIT.to_owned());
                }
            }
            let scalar_type = native_field(&declaration, SCALAR_TYPE_ATTRIBUTE)?;
            fact.data_type = scalar_type.extract()?;
            resolved_columns.push(column.enum_type(py, scalar_type, generated)?);
            facts.push(fact);
            continue;
        }
        let mut audits: Vec<Bound<'py, PyAny>> = column
            .field(py, AUDITS_FIELD)?
            .try_iter()?
            .collect::<PyResult<_>>()?;
        if enforced {
            let generated: Bound<'py, PyAny> = accepted_values(context, &declaration)?;
            let mut present: bool = false;
            for audit in &audits {
                if audit.eq(&generated)? {
                    present = true;
                    break;
                }
            }
            if !present {
                audits.push(generated);
                fact.audit_definitions
                    .push(ACCEPTED_VALUES_AUDIT.to_owned());
            }
        }
        let scalar_type: Bound<'py, PyAny> = native_field(&declaration, SCALAR_TYPE_ATTRIBUTE)?;
        fact.data_type = scalar_type.extract()?;
        resolved_columns.push(HeaderColumn::replaced(
            &context.contracts.schema_column,
            [
                column.field(py, "name")?,
                scalar_type,
                column.field(py, "nullable")?,
                column.field(py, "description")?,
                column.field(py, "meta")?,
                PyTuple::new(py, audits)?.into_any(),
                column.field(py, "location")?,
                column.field(py, "migrate_from")?,
            ],
            [0, 2, 3, 4, 6, 7, 1, 5],
        )?);
        facts.push(fact);
    }
    let usages: Vec<Bound<'py, PyAny>> =
        generated_usages(context, declarations, consumer, &resolved)?;
    Ok(EnumColumns {
        columns: Arc::new(HeaderColumns::new(resolved_columns)),
        facts,
        declarations: resolved,
        usages,
    })
}

fn accepted_values<'py>(
    context: &LoopContext<'_, 'py>,
    declaration: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = declaration.py();
    let mut members: Vec<Bound<'py, PyAny>> = Vec::new();
    for member in native_field(declaration, MEMBERS_FIELD)?.try_iter()? {
        members.push(native_field(&member?, VALUE_FIELD)?);
    }
    let arguments: Bound<'py, PyDict> = PyDict::new(py);
    arguments.set_item(VALUES_ARGUMENT, PyTuple::new(py, members)?)?;
    keywords(
        &context.contracts.schema_audit_instance,
        &[
            (
                DEFINITION_NAME_FIELD,
                ACCEPTED_VALUES_AUDIT.into_pyobject_any(py)?,
            ),
            (ARGUMENTS_FIELD, arguments.into_any()),
        ],
    )
}

fn generated_usages<'py>(
    context: &LoopContext<'_, 'py>,
    declarations: &Bound<'py, PyAny>,
    consumer: &Bound<'py, PyAny>,
    resolved: &Bound<'py, PyDict>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let visibility: Bound<'py, PyDict> =
        native_field(declarations, ENUM_VISIBILITY_ATTRIBUTE)?.downcast_into()?;
    let mut usages: Vec<Bound<'py, PyAny>> = Vec::new();
    for declaration in resolved.values() {
        let Some(records) = visibility.get_item(native_field(&declaration, NAME_FIELD)?)? else {
            continue;
        };
        for record in records.try_iter()? {
            let record: Bound<'py, PyAny> = record?;
            usages.push(keywords(
                &context.classes.usage_record,
                &[
                    (CONSUMER_FIELD, consumer.clone()),
                    (DECLARATION_FIELD, native_field(&record, DECLARATION_FIELD)?),
                    (KIND_FIELD, context.classes.generated_usage.clone()),
                    (THROUGH_FIELD, native_field(&record, THROUGH_FIELD)?),
                ],
            )?);
        }
    }
    Ok(usages)
}

fn keywords<'py>(
    class: &Bound<'py, PyAny>,
    fields: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: Bound<'py, PyDict> = PyDict::new(class.py());
    for (key, value) in fields {
        arguments.set_item(*key, value)?;
    }
    class.call((), Some(&arguments))
}

/// `into_pyobject` for any value, as a `PyAny`.
trait IntoPyAny<'py> {
    fn into_pyobject_any(self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>>;
}

impl<'py, T> IntoPyAny<'py> for T
where
    T: pyo3::IntoPyObject<'py>,
    PyErr: From<T::Error>,
{
    fn into_pyobject_any(self, py: Python<'py>) -> PyResult<Bound<'py, PyAny>> {
        Ok(self.into_pyobject(py)?.into_any().into_bound())
    }
}
