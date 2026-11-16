//! Contract adoption's comparison of authored contracts with physical warehouse columns.

use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_core::text::main::python_casefold::python_casefold;
use sqlbuild_host::entries::models::AdapterHost;

use crate::bindings::_helpers::contracts::spec_resolution::matching_dynamic_families;
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::fields::replaced::replaced;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_base::type_system::main::types_equal::types_equal as native_types_equal;

const MODEL_RESOURCE: &str = "model";
const SOURCE_RESOURCE: &str = "source";

/// Python callables and classes the comparison builds findings with.
struct Vocabulary<'py> {
    finding: Bound<'py, PyAny>,
    kinds: Bound<'py, PyAny>,
}

impl<'py> Vocabulary<'py> {
    fn new(py: Python<'py>) -> PyResult<Self> {
        Ok(Self {
            finding: record_class(py, RecordId::ContractFinding)?,
            kinds: record_class(py, RecordId::ContractFindingKind)?,
        })
    }

    fn py(&self) -> Python<'py> {
        self.finding.py()
    }

    /// One `ContractFinding` about `item` with `kind` and the given optional fields.
    fn finding(
        &self,
        item: &Bound<'py, PyAny>,
        kind: &str,
        fields: Vec<(&str, Bound<'py, PyAny>)>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let keywords: Bound<'py, PyDict> = PyDict::new(self.py());
        keywords.set_item("resource_type", native_field(item, "resource_type")?)?;
        keywords.set_item("resource_name", native_field(item, "resource_name")?)?;
        keywords.set_item("kind", self.kinds.getattr(kind)?)?;
        for (name, value) in fields {
            keywords.set_item(name, value)?;
        }
        self.finding.call((), Some(&keywords))
    }

    /// `replace(item, **fields)`.
    fn replaced(
        &self,
        item: &Bound<'py, PyAny>,
        fields: Vec<(&str, Bound<'py, PyAny>)>,
    ) -> PyResult<Bound<'py, PyAny>> {
        replaced(item, &fields)
    }
}

fn text<'py>(py: Python<'py>, value: &str) -> Bound<'py, PyAny> {
    PyString::new(py, value).into_any()
}

fn keyword_call<'py>(
    callable: &Bound<'py, PyAny>,
    pairs: Vec<(&str, Bound<'py, PyAny>)>,
) -> PyResult<Bound<'py, PyAny>> {
    let keywords: Bound<'py, PyDict> = PyDict::new(callable.py());
    for (name, value) in pairs {
        keywords.set_item(name, value)?;
    }
    callable.call((), Some(&keywords))
}

/// `database.schema.relation` with the absent parts left out.
fn qualified(item: &Bound<'_, PyAny>) -> PyResult<String> {
    let mut parts: Vec<String> = Vec::new();
    for name in ["database", "schema", "relation"] {
        let part: Option<String> = native_field(item, name)?.extract()?;
        if let Some(part) = part.filter(|part| !part.is_empty()) {
            parts.push(part);
        }
    }
    Ok(parts.join("."))
}

fn column_infos<'py>(
    py: Python<'py>,
    columns: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let column_type: Bound<'py, PyAny> = host_entry(py, AdapterHost::ColumnInfo)?;
    let mut infos: Vec<Bound<'py, PyAny>> = Vec::new();
    for column in columns.try_iter()? {
        let column: Bound<'py, PyAny> = column?;
        let declared: Option<String> = native_field(&column, "type")?.extract()?;
        infos.push(keyword_call(
            &column_type,
            vec![
                ("name", native_field(&column, "name")?),
                ("type", text(py, &declared.unwrap_or_default())),
            ],
        )?);
    }
    PyTuple::new(py, infos)
}

fn resource_type<'py>(py: Python<'py>, value: &str) -> PyResult<Bound<'py, PyAny>> {
    enum_from_value(py, RecordId::CompiledResourceType, value)
}

/// The evidence a model's authored contract offers, before inspection.
fn model_evidence<'py>(model: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.py();
    let destination: Bound<'py, PyAny> = native_field(model, "destination")?;
    let schema_entry: Bound<'py, PyAny> = native_field(model, "schema_entry")?;
    let (declared, dynamic): (Bound<'py, PyAny>, Bound<'py, PyAny>) = if schema_entry.is_none() {
        (PyTuple::empty(py).into_any(), PyTuple::empty(py).into_any())
    } else {
        (
            column_infos(py, &native_field(&schema_entry, "columns")?)?.into_any(),
            native_field(&schema_entry, "dynamic_columns")?,
        )
    };
    keyword_call(
        &record_class(py, RecordId::ContractEvidence)?,
        vec![
            ("resource_type", resource_type(py, MODEL_RESOURCE)?),
            ("resource_name", native_field(model, "name")?),
            ("database", native_field(&destination, "database")?),
            ("schema", native_field(&destination, "schema")?),
            ("relation", native_field(&destination, "name")?),
            ("declared_columns", declared),
            ("physical_columns", py.None().into_bound(py)),
            ("source_path", native_field(model, "relative_path")?),
            ("dynamic_columns", dynamic),
        ],
    )
}

/// The evidence a source's authored contract offers; expressions cannot be inspected.
fn source_evidence<'py>(
    vocabulary: &Vocabulary<'py>,
    source: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = source.py();
    let entry: Bound<'py, PyAny> = native_field(source, "source_entry")?;
    let table: Option<String> = native_field(&entry, "table")?.extract()?;
    let relation: String = match table.filter(|table| !table.is_empty()) {
        Some(table) => table,
        None => native_field(&entry, "name")?.extract()?,
    };
    let name: Bound<'py, PyAny> = native_field(source, "name")?;
    let inspectable: bool = native_field(&entry, "expression")?.is_none() && !relation.is_empty();
    let resource: Bound<'py, PyAny> = resource_type(py, SOURCE_RESOURCE)?;
    let mut findings: Vec<Bound<'py, PyAny>> = Vec::new();
    if !inspectable {
        let keywords: Bound<'py, PyDict> = PyDict::new(py);
        keywords.set_item("resource_type", &resource)?;
        keywords.set_item("resource_name", &name)?;
        keywords.set_item(
            "kind",
            native_field(&vocabulary.kinds, "INSPECTION_UNAVAILABLE")?,
        )?;
        keywords.set_item(
            "message",
            "source expression has no inspectable physical relation",
        )?;
        findings.push(vocabulary.finding.call((), Some(&keywords))?);
    }
    let relation: Bound<'py, PyAny> = if relation.is_empty() {
        name.clone()
    } else {
        text(py, &relation)
    };
    keyword_call(
        &record_class(py, RecordId::ContractEvidence)?,
        vec![
            ("resource_type", resource),
            ("resource_name", name),
            ("database", native_field(&entry, "database")?),
            ("schema", native_field(&entry, "schema")?),
            ("relation", relation),
            (
                "declared_columns",
                column_infos(py, &native_field(&entry, "columns")?)?.into_any(),
            ),
            ("physical_columns", py.None().into_bound(py)),
            (
                "source_path",
                native_field(&native_field(source, "source_file")?, "relative_path")?,
            ),
            ("findings", PyTuple::new(py, findings)?.into_any()),
        ],
    )
}

/// Evidence whose relation could not be inspected.
fn unavailable_evidence<'py>(
    vocabulary: &Vocabulary<'py>,
    item: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = vocabulary.py();
    let finding: Bound<'py, PyAny> = vocabulary.finding(
        item,
        "INSPECTION_UNAVAILABLE",
        vec![(
            "message",
            text(
                py,
                &format!(
                    "physical relation {} could not be inspected",
                    qualified(item)?
                ),
            ),
        )],
    )?;
    vocabulary.replaced(
        item,
        vec![("findings", PyTuple::new(py, [finding])?.into_any())],
    )
}

fn types_equal(
    adapter: &Bound<'_, PyAny>,
    (left, right): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
) -> PyResult<bool> {
    native_types_equal(
        &adapter.call_method0("sql_analysis_dialect")?,
        &left.extract::<String>()?,
        &right.extract::<String>()?,
    )
}

/// The finding for a physical column no declared column names, if any.
fn undeclared_finding<'py>(
    vocabulary: &Vocabulary<'py>,
    (item, physical): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    adapter: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py: Python<'py> = vocabulary.py();
    let name: String = native_field(physical, "name")?.extract()?;
    let physical_type: Bound<'py, PyAny> = native_field(physical, "type")?;
    let matching: Bound<'py, PyAny> =
        matching_dynamic_families(&native_field(item, "dynamic_columns")?, &name)?.into_any();
    if matching.len()? == 1 {
        let family: Bound<'py, PyAny> = matching.get_item(0)?;
        let family_type: Bound<'py, PyAny> = native_field(&family, "type")?;
        if types_equal(adapter, (&family_type, &physical_type))? {
            return Ok(None);
        }
        let message: String = format!(
            "dynamic column '{name}' is physically {} but family '{}' declares {}",
            physical_type.str()?,
            native_field(&family, "name")?.str()?,
            family_type.str()?
        );
        return vocabulary
            .finding(
                item,
                "TYPE_MISMATCH",
                vec![
                    ("column_name", text(py, &name)),
                    ("declared_type", family_type),
                    ("physical_type", physical_type),
                    ("message", text(py, &message)),
                ],
            )
            .map(Some);
    }
    let message: String = format!(
        "physical column '{name}' ({}) is not declared",
        physical_type.str()?
    );
    vocabulary
        .finding(
            item,
            "MISSING_DECLARATION",
            vec![
                ("column_name", text(py, &name)),
                ("physical_type", physical_type),
                ("message", text(py, &message)),
            ],
        )
        .map(Some)
}

/// The finding for a physical column a declared column names, if any.
fn declared_finding<'py>(
    vocabulary: &Vocabulary<'py>,
    (item, declared, physical): (&Bound<'py, PyAny>, &Bound<'py, PyAny>, &Bound<'py, PyAny>),
    adapter: &Bound<'py, PyAny>,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let py: Python<'py> = vocabulary.py();
    let declared_name: Bound<'py, PyAny> = native_field(declared, "name")?;
    let declared_type: Bound<'py, PyAny> = native_field(declared, "type")?;
    let physical_type: Bound<'py, PyAny> = native_field(physical, "type")?;
    if !declared_type.is_truthy()? {
        let message: String = format!(
            "declared column '{}' has no type; physical type is {}",
            declared_name.str()?,
            physical_type.str()?
        );
        return vocabulary
            .finding(
                item,
                "MISSING_DECLARATION",
                vec![
                    ("column_name", native_field(physical, "name")?),
                    ("physical_type", physical_type),
                    ("message", text(py, &message)),
                ],
            )
            .map(Some);
    }
    if types_equal(adapter, (&declared_type, &physical_type))? {
        return Ok(None);
    }
    let message: String = format!(
        "column '{}' is declared {} but physical type is {}",
        declared_name.str()?,
        declared_type.str()?,
        physical_type.str()?
    );
    vocabulary
        .finding(
            item,
            "TYPE_MISMATCH",
            vec![
                ("column_name", declared_name),
                ("declared_type", declared_type),
                ("physical_type", physical_type),
                ("message", text(py, &message)),
            ],
        )
        .map(Some)
}

/// `item` with its physical columns and every difference from its authored contract.
fn compared<'py>(
    vocabulary: &Vocabulary<'py>,
    (item, physical_columns): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    adapter: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = vocabulary.py();
    if physical_columns.is_none() {
        let finding: Bound<'py, PyAny> = vocabulary.finding(
            item,
            "RELATION_MISSING",
            vec![(
                "message",
                text(
                    py,
                    &format!("physical relation {} does not exist", qualified(item)?),
                ),
            )],
        )?;
        return vocabulary.replaced(
            item,
            vec![("findings", PyTuple::new(py, [finding])?.into_any())],
        );
    }
    let declared: Vec<Bound<'py, PyAny>> = native_field(item, "declared_columns")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    let physical: Vec<Bound<'py, PyAny>> = physical_columns.try_iter()?.collect::<PyResult<_>>()?;
    let mut declared_keys: Vec<String> = Vec::with_capacity(declared.len());
    for column in &declared {
        declared_keys.push(python_casefold(
            &native_field(column, "name")?.extract::<String>()?,
        ));
    }
    let mut physical_keys: Vec<String> = Vec::with_capacity(physical.len());
    for column in &physical {
        physical_keys.push(python_casefold(
            &native_field(column, "name")?.extract::<String>()?,
        ));
    }
    let mut findings: Vec<Bound<'py, PyAny>> = Vec::new();
    for (column, key) in physical.iter().zip(&physical_keys) {
        let matched: Option<&Bound<'py, PyAny>> = declared_keys
            .iter()
            .rposition(|declared_key| declared_key == key)
            .and_then(|position| declared.get(position));
        let finding: Option<Bound<'py, PyAny>> = match matched {
            None => undeclared_finding(vocabulary, (item, column), adapter)?,
            Some(declared_column) => {
                declared_finding(vocabulary, (item, declared_column, column), adapter)?
            }
        };
        findings.extend(finding);
    }
    for (column, key) in declared.iter().zip(&declared_keys) {
        if physical_keys.contains(key) {
            continue;
        }
        let name: Bound<'py, PyAny> = native_field(column, "name")?;
        let declared_type: Bound<'py, PyAny> = native_field(column, "type")?;
        let message: String = format!(
            "declared column '{}' is absent from the physical relation",
            name.str()?
        );
        let declared_type: Bound<'py, PyAny> = if declared_type.is_truthy()? {
            declared_type
        } else {
            py.None().into_bound(py)
        };
        findings.push(vocabulary.finding(
            item,
            "MISSING_PHYSICAL_COLUMN",
            vec![
                ("column_name", name),
                ("declared_type", declared_type),
                ("message", text(py, &message)),
            ],
        )?);
    }
    vocabulary.replaced(
        item,
        vec![
            ("physical_columns", physical_columns.clone()),
            ("findings", PyTuple::new(py, findings)?.into_any()),
        ],
    )
}

/// `item` compared with its physical columns, or marked missing when there are none.
#[pyfunction]
#[pyo3(signature = (*, item, physical_columns, adapter))]
fn compare_contract_evidence<'py>(
    py: Python<'py>,
    item: &Bound<'py, PyAny>,
    physical_columns: &Bound<'py, PyAny>,
    adapter: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    compared(&Vocabulary::new(py)?, (item, physical_columns), adapter)
}

fn relation_info<'py>(item: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = item.py();
    keyword_call(
        &host_entry(py, AdapterHost::RelationInfo)?,
        vec![
            ("database", native_field(item, "database")?),
            ("schema", native_field(item, "schema")?),
            ("name", native_field(item, "relation")?),
            ("relation_type", text(py, "")),
        ],
    )
}

/// Selected declarations compared with one bulk read of physical columns.
#[pyfunction]
#[pyo3(signature = (**request))]
pub(crate) fn compare_contracts<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let request = Request::new("compare_contracts", request)?;
    let py: Python<'py> = request.py();
    let adapter: &Bound<'py, PyAny> = &request.get("adapter")?;
    let connection: Bound<'py, PyAny> = request.get("connection")?;
    let models: Bound<'py, PyAny> = request.get("models")?;
    let sources: Bound<'py, PyAny> = request.get("sources")?;
    let vocabulary: Vocabulary<'py> = Vocabulary::new(py)?;
    let mut inspectable: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut evidence: Vec<Bound<'py, PyAny>> = Vec::new();
    for model in models.try_iter()? {
        inspectable.push(model_evidence(&model?)?);
    }
    for source in sources.try_iter()? {
        let item: Bound<'py, PyAny> = source_evidence(&vocabulary, &source?)?;
        if native_field(&item, "findings")?.is_truthy()? {
            evidence.push(item);
        } else {
            inspectable.push(item);
        }
    }
    let relations: Vec<Bound<'py, PyAny>> = inspectable
        .iter()
        .map(relation_info)
        .collect::<PyResult<_>>()?;
    let keywords: Bound<'py, PyDict> = PyDict::new(py);
    keywords.set_item("connection", connection)?;
    keywords.set_item("relations", PyTuple::new(py, &relations)?)?;
    let physical: Bound<'py, PyAny> =
        match adapter.call_method("get_columns_for_relations", (), Some(&keywords)) {
            Ok(physical) => physical,
            Err(error) if error.is_instance_of::<PyException>(py) => {
                for item in &inspectable {
                    evidence.push(unavailable_evidence(&vocabulary, item)?);
                }
                return PyTuple::new(py, evidence);
            }
            Err(error) => return Err(error),
        };
    for (item, relation) in inspectable.iter().zip(&relations) {
        let columns: Bound<'py, PyAny> =
            mapping_get(&physical, native_field(relation, "identity")?)?;
        evidence.push(compared(&vocabulary, (item, &columns), adapter)?);
    }
    PyTuple::new(py, evidence)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(compare_contracts, module)?)?;
    module.add_function(wrap_pyfunction!(compare_contract_evidence, module)?)?;
    Ok(())
}
