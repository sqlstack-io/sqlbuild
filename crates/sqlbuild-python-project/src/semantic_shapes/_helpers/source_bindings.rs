//! Rebind selected models to source columns the planner already inspected.

use crate::semantic_shapes::main::declared_types::declared_types;
use crate::semantic_shapes::main::function_names::function_names;
use std::collections::{BTreeSet, HashMap, HashSet};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::{PyBool, PyDict, PyDictMethods, PyString, PyTuple};

use crate::analysis::constants::CASE_SENSITIVE_BINDING_DIALECTS;
use crate::analysis::constants::NATIVE_DIALECT_ALIASES;
use crate::analysis::main::schema_validation_request::schema_validation_request;
use crate::analysis::main::schema_validations::schema_validations;
use crate::analysis::models::BindingPositions;
use crate::analysis::models::SchemaRequestParts;
use crate::analysis_session::main::infer_native_query_columns::infer_native_query_columns;
use crate::semantic_shapes::_helpers::shapes::model_inputs;
use crate::semantic_shapes::_helpers::shapes::project_shapes;
use crate::semantic_shapes::_helpers::shapes::published_shape;
use crate::semantic_shapes::constants::SOURCE_REBINDING_ANALYSES;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::compile_driver::main::count_stage::count_stage;
use sqlbuild_python_base::records::main::construct::enum_from_value::enum_from_value;
use sqlbuild_python_base::records::main::construct::enum_member::enum_member;
use sqlbuild_python_base::records::main::construct::record_class::record_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_base::records::main::fields::replaced_keywords::replaced_keywords;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::attachments::main::analysis_cursor_sql::analysis_cursor_sql;
use sqlbuild_analysis::analysis_route::main::same_column_type::same_column_type;
use sqlbuild_analysis::source_drift::main::reads_added_columns::reads_added_columns;
use sqlbuild_sqltext::resolution::main::unquoted_column_name::unquoted_column_name;

const BINDING_UNKNOWN_TABLE_INTERNAL_CODE: &str = "B000";
const GRAPHLIB_MODULE: &str = "graphlib";
const REF_KIND: &str = "ref";
const SOURCE_KIND: &str = "source";
const UNKNOWN: &str = "UNKNOWN";
const GENERIC_DIALECT: &str = "generic";
/// Dialects whose unquoted identifiers already bind with their exact case.
const EXACT_UNQUOTED_DIALECTS: [&str; 1] = ["clickhouse"];
/// The upper-case folding dialect; the adapter lists folded names lowercased and others quoted.
const UPPERCASE_FOLDING_DIALECT: &str = "snowflake";

type Object<'py> = Bound<'py, PyAny>;

/// `source_binding_diagnostics(*, project, columns, profile, selected_keys)`.
#[pyfunction]
#[pyo3(signature = (**request))]
pub(crate) fn source_binding_diagnostics<'py>(
    request: Option<&Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyTuple>> {
    let request = Request::new("source_binding_diagnostics", request)?;
    let py: Python<'py> = request.py();
    let project: Object<'py> = request.get("project")?;
    let columns: Object<'py> = request.get("columns")?;
    if !native_field(&native_field(&project, "settings")?, "sql_analysis")?.is_truthy()?
        || !columns.is_truthy()?
    {
        return Ok(PyTuple::empty(py));
    }
    let profile: Object<'py> = request.get("profile")?;
    let raw_dialect: Object<'py> = native_field(&profile, "sql_analysis_dialect")?;
    let key: Object<'py> = if raw_dialect.is_truthy()? {
        raw_dialect.clone()
    } else {
        PyString::new(py, GENERIC_DIALECT).into_any()
    };
    let key: String = key.extract()?;
    let dialect: Object<'py> = match NATIVE_DIALECT_ALIASES
        .iter()
        .find(|(alias, _)| *alias == key)
    {
        Some((_, native)) => PyString::new(py, native).into_any(),
        None => raw_dialect.clone(),
    };
    let ignore_case: Object<'py> = ignores_quoted_case(&project, &raw_dialect)?;
    let profile: Object<'py> = replaced(
        &profile,
        &[
            ("sql_analysis_dialect", dialect.clone()),
            ("quoted_identifiers_ignore_case", ignore_case.clone()),
        ],
    )?;
    let shapes: Bound<'py, PyDict> = project_shapes(&project, Some(profile.clone()))?;
    let sensitive: bool = !dialect.is_none()
        && CASE_SENSITIVE_BINDING_DIALECTS.contains(&dialect.extract::<String>()?.as_str())
        && !ignore_case.is_truthy()?;
    let exact_unquoted: bool = !dialect.is_none()
        && EXACT_UNQUOTED_DIALECTS.contains(&dialect.extract::<String>()?.as_str());
    let folds_upper: bool =
        !dialect.is_none() && dialect.extract::<String>()?.as_str() == UPPERCASE_FOLDING_DIALECT;
    let comparison: Comparison = Comparison {
        dialect: dialect.extract()?,
        sensitive,
        folds_upper,
    };
    let mut changed: HashMap<String, Drift> = HashMap::new();
    for item in mapping_items(&columns)?.try_iter()? {
        let (name, value): (String, Object<'py>) = item?.extract()?;
        if !value.is_truthy()? {
            continue;
        }
        let physical: Vec<(String, Option<String>)> = value
            .try_iter()?
            .map(|column| {
                let column: Object<'py> = column?;
                Ok((
                    native_field(&column, "name")?.extract()?,
                    native_field(&column, "type")?.extract()?,
                ))
            })
            .collect::<PyResult<_>>()?;
        let bound: Vec<(String, Option<String>)> = physical
            .iter()
            .map(|(column, column_type)| {
                let listed_quoted: bool = folds_upper && unquoted_column_name(column) != column;
                let name: String = if exact_unquoted
                    || listed_quoted
                    || (folds_upper && is_folded_identifier(column))
                {
                    column.clone()
                } else {
                    format!("\"{}\"", column.replace('"', "\"\""))
                };
                (name, column_type.clone())
            })
            .collect();
        let known: Option<Object<'py>> = shapes.get_item(&name)?;
        let listed: &[(String, Option<String>)] = if sensitive { &bound } else { &physical };
        if let Some(drift) = comparison.drift(listed, known.as_ref())? {
            changed.insert(name.clone(), drift);
        }
        let quoted: Bound<'py, PyDict> = PyDict::new(py);
        for (column, column_type) in bound {
            quoted.set_item(column, column_type)?;
        }
        shapes.set_item(name, quoted)?;
    }
    let binding_catalog: Object<'py> = native_field(&project, "binding_catalog")?;
    let mut physical_catalog: Object<'py> = if binding_catalog.is_none() {
        binding_catalog
    } else {
        binding_catalog.call_method1("with_relations", (&shapes,))?
    };
    let profile: Object<'py> =
        replaced(&profile, &[("binding_catalog", physical_catalog.clone())])?;
    let models: Vec<Object<'py>> = rebound_models(
        &project,
        &request.get("selected_keys")?,
        (&changed, &shapes),
        (&profile, &comparison),
    )?;
    if !physical_catalog.is_none() {
        physical_catalog = physical_catalog.call_method1("with_relations", (&shapes,))?;
    }
    validated_diagnostics(&project, &models, (&shapes, &physical_catalog), &dialect)
}

/// Selected models whose result inspected columns could change, re-analysed in dependency order.
fn rebound_models<'py>(
    project: &Object<'py>,
    selected_keys: &Object<'py>,
    (changed, shapes): (&HashMap<String, Drift>, &Bound<'py, PyDict>),
    (profile, comparison): (&Object<'py>, &Comparison),
) -> PyResult<Vec<Object<'py>>> {
    let py: Python<'py> = project.py();
    let (by_name, order) = dependency_order(project)?;
    let query_class: Object<'py> = record_class(py, RecordId::NativeColumnQuery)?;
    let unknown: Object<'py> = enum_member(py, RecordId::InferredNullability, "UNKNOWN")?;
    let mut affected: HashMap<String, Drift> = HashMap::new();
    let mut models: Vec<Object<'py>> = Vec::new();
    for name in order.try_iter()? {
        let model: Object<'py> = by_name.as_any().get_item(name?)?;
        let opted: Object<'py> = native_field(&native_field(&model, "config")?, "values")?
            .call_method1("get", ("sql_analysis",))?;
        if !selected_keys.contains(native_field(&model, "key")?)?
            || opted.is(PyBool::new(py, false))
        {
            continue;
        }
        if !needs_reanalysis(&model, (changed, &affected), comparison)? {
            continue;
        }
        let model_name: String = native_field(&model, "name")?.extract()?;
        let known: Option<Object<'py>> = shapes.get_item(&model_name)?;
        models.push(model.clone());
        let inputs: Bound<'py, PyDict> = model_inputs(&model, shapes)?;
        let nullability: Bound<'py, PyDict> = PyDict::new(py);
        for (table, shape) in inputs.iter() {
            let defaults: Object<'py> = PyDict::new(py)
                .into_any()
                .getattr("fromkeys")?
                .call1((shape, &unknown))?;
            nullability.set_item(table, defaults)?;
        }
        let query: Object<'py> = call(
            &query_class,
            &[
                ("sql", native_field(&model, "query_sql")?),
                ("mode", PyString::new(py, "reanalysis").into_any()),
                ("placeholders", placeholders(&model)?),
                ("references", native_field(&model, "references")?),
            ],
        )?;
        count_stage(py, SOURCE_REBINDING_ANALYSES, 1)?;
        let analysis: Object<'py> = infer_native_query_columns(Some(&keywords(
            py,
            &[
                ("queries", PyTuple::new(py, [query])?.into_any()),
                ("profile", profile.clone()),
                ("column_types_by_table", inputs.clone().into_any()),
                ("column_nullability_by_table", nullability.into_any()),
            ],
        )?))?
        .get_item(0)?;
        let inferred: Object<'py> = native_field(&analysis, "columns")?;
        let mut all_inputs_closed: bool = true;
        for shape in inputs.values() {
            if !shape.is_truthy()? {
                all_inputs_closed = false;
                break;
            }
        }
        if inferred.is_truthy()?
            && (!native_field(&analysis, "has_star")?.is_truthy()? || all_inputs_closed)
        {
            let columns: Bound<'py, PyDict> = PyDict::new(py);
            for column in inferred.try_iter()? {
                let column: Object<'py> = column?;
                let column_type: Object<'py> = native_field(&column, "type")?;
                let column_type: Object<'py> = if column_type.is_truthy()? {
                    column_type
                } else {
                    PyString::new(py, UNKNOWN).into_any()
                };
                columns.set_item(native_field(&column, "name")?, column_type)?;
            }
            let shape: Bound<'py, PyDict> = published_shape(&model, &columns, &inputs, profile)?;
            let listed: Vec<(String, Option<String>)> = mapping_items(&shape)?
                .try_iter()?
                .map(|item| item?.extract())
                .collect::<PyResult<_>>()?;
            if let Some(drift) = comparison.drift(&listed, known.as_ref())? {
                affected.insert(model_name.clone(), drift);
            }
            shapes.set_item(&model_name, shape)?;
        } else {
            affected.insert(model_name.clone(), Drift::Changed);
            if shapes.contains(&model_name)? {
                shapes.del_item(&model_name)?;
            }
        }
    }
    Ok(models)
}

/// The project's models by name, and their names in dependency order.
fn dependency_order<'py>(project: &Object<'py>) -> PyResult<(Bound<'py, PyDict>, Object<'py>)> {
    let py: Python<'py> = project.py();
    let by_name: Bound<'py, PyDict> = PyDict::new(py);
    for model in native_field(project, "models")?.try_iter()? {
        let model: Object<'py> = model?;
        by_name.set_item(native_field(&model, "name")?, &model)?;
    }
    let graph: Bound<'py, PyDict> = PyDict::new(py);
    for model in native_field(project, "models")?.try_iter()? {
        let model: Object<'py> = model?;
        let mut upstream: Vec<Object<'py>> = Vec::new();
        for reference in native_field(&model, "references")?.try_iter()? {
            let reference: Object<'py> = reference?;
            let name: Object<'py> = native_field(&reference, "ref_name")?;
            if native_field(&reference, "ref_kind")?.eq(REF_KIND)? && by_name.contains(&name)? {
                upstream.push(name);
            }
        }
        graph.set_item(native_field(&model, "name")?, PyTuple::new(py, upstream)?)?;
    }
    let order: Object<'py> = py
        .import(GRAPHLIB_MODULE)?
        .getattr("TopologicalSorter")?
        .call1((graph,))?
        .call_method0("static_order")?;
    Ok((by_name, order))
}

/// Whether `model` reads a changed input, or inputs whose added columns could change its result.
fn needs_reanalysis(
    model: &Object<'_>,
    (changed, affected): (&HashMap<String, Drift>, &HashMap<String, Drift>),
    comparison: &Comparison,
) -> PyResult<bool> {
    let mut drifted: Vec<String> = Vec::new();
    let mut added: BTreeSet<String> = BTreeSet::new();
    for reference in native_field(model, "references")?.try_iter()? {
        let reference: Object<'_> = reference?;
        let kind: String = native_field(&reference, "ref_kind")?.str()?.extract()?;
        let upstream: String = native_field(&reference, "ref_name")?.extract()?;
        let drift: Option<&Drift> = match kind.as_str() {
            SOURCE_KIND => changed.get(&upstream),
            REF_KIND => affected.get(&upstream),
            _ => None,
        };
        match drift {
            Some(Drift::Changed) => return Ok(true),
            Some(Drift::Added(names)) => {
                drifted.push(upstream);
                added.extend(names.iter().cloned());
            }
            None => {}
        }
    }
    if drifted.is_empty() {
        return Ok(false);
    }
    comparison.reads_added(model, &drifted, &added.into_iter().collect::<Vec<_>>())
}

/// Each rebound model's binding diagnostics, located in its authored file.
fn validated_diagnostics<'py>(
    project: &Object<'py>,
    models: &[Object<'py>],
    (shapes, catalog): (&Bound<'py, PyDict>, &Object<'py>),
    dialect: &Object<'py>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = project.py();
    let functions: Object<'py> = native_field(project, "functions")?;
    let known_functions: Object<'py> = PyTuple::new(py, function_names(&functions)?)?.into_any();
    let known_types: Object<'py> =
        PyTuple::new(py, declared_types(&functions, shapes.as_any())?)?.into_any();
    let ignores: bool = crate::analysis::main::ignores_quoted_case::ignores_quoted_case(
        &native_field(project, "effective_connection")?,
        dialect,
    )?;
    let mut requests: Vec<Object<'py>> = Vec::with_capacity(models.len());
    for model in models {
        let query_sql: String = analysis_cursor_sql(
            py,
            &native_field(model, "query_sql")?.extract::<String>()?,
            &native_field(&native_field(model, "config")?, "values")?
                .call_method1("get", ("cursor_type",))?,
        )?;
        requests.push(schema_validation_request(
            py,
            SchemaRequestParts {
                query_sql,
                placeholders: placeholders(model)?,
                dialect: dialect.clone(),
                schema: model_inputs(model, shapes)?.into_any(),
                known_functions: known_functions.clone(),
                known_types: known_types.clone(),
                catalog: catalog.clone(),
                quoted_identifiers_ignore_case: ignores,
            },
        )?);
    }
    let results: Object<'py> =
        schema_validations(py, PyTuple::new(py, &requests)?.as_any())?.into_any();
    let expansions: Bound<'py, PyDict> = PyDict::new(py);
    for item in native_field(project, "sql_expansions")?
        .call_method0("items")?
        .try_iter()?
    {
        let (path, expansion): (Object<'py>, Object<'py>) = item?.extract()?;
        expansions.set_item(native_field(&path, "stem")?, expansion)?;
    }
    let skipped: Object<'py> = PyString::new(py, BINDING_UNKNOWN_TABLE_INTERNAL_CODE).into_any();
    let diagnostic_class: Object<'py> = record_class(py, RecordId::CompilerDiagnostic)?;
    let phase: Object<'py> = enum_member(py, RecordId::DiagnosticPhase, "COMPILE")?;
    let model_type: Object<'py> = enum_member(py, RecordId::CompiledResourceType, "MODEL")?;
    let mut diagnostics: Vec<Object<'py>> = Vec::new();
    for ((model, request), result) in models.iter().zip(&requests).zip(results.try_iter()?) {
        let result: Object<'py> = result?;
        let expansion: Option<Object<'py>> = expansions.get_item(native_field(model, "name")?)?;
        let mut mapper: Option<Object<'py>> = None;
        for diagnostic in native_field(&result, "diagnostics")?.try_iter()? {
            let diagnostic: Object<'py> = diagnostic?;
            if native_field(&diagnostic, "code")?.eq(&skipped)? {
                continue;
            }
            let built: Object<'py> = match mapper.take() {
                Some(built) => built,
                None => positions(model, &native_field(request, "sql")?, expansion.as_ref())?,
            };
            let position: Object<'py> = native_field(&built, "position")?;
            mapper = Some(built);
            let (line, column): (Object<'py>, Object<'py>) = call(
                &position,
                &[
                    ("start", native_field(&diagnostic, "start")?),
                    ("line", native_field(&diagnostic, "line")?),
                    ("column", native_field(&diagnostic, "column")?),
                    ("message", native_field(&diagnostic, "message")?),
                ],
            )?
            .extract()?;
            diagnostics.push(call(
                &diagnostic_class,
                &[
                    ("phase", phase.clone()),
                    (
                        "severity",
                        enum_from_value(
                            py,
                            RecordId::DiagnosticSeverity,
                            native_field(&diagnostic, "severity")?,
                        )?,
                    ),
                    ("code", native_field(&diagnostic, "code")?),
                    ("message", native_field(&diagnostic, "message")?),
                    ("resource_type", model_type.clone()),
                    ("resource_name", native_field(model, "name")?),
                    ("path", native_field(model, "relative_path")?),
                    ("line", line),
                    ("column", column),
                ],
            )?);
        }
    }
    PyTuple::new(py, diagnostics)
}

/// The native position mapper from cleaned binding SQL back to the authored model file.
fn positions<'py>(
    model: &Object<'py>,
    cleaned: &Object<'py>,
    expansion: Option<&Object<'py>>,
) -> PyResult<Object<'py>> {
    let py: Python<'py> = model.py();
    let authored_query: Object<'py> = native_field(model, "authored_query_sql")?;
    let mut passes: Vec<Object<'py>> = Vec::new();
    let expanded: Object<'py> = match expansion {
        None => authored_query.clone(),
        Some(expansion) => {
            for spans in native_field(expansion, "passes")?.try_iter()? {
                let mut rows: Vec<Object<'py>> = Vec::new();
                for span in spans?.try_iter()? {
                    let span: Object<'py> = span?;
                    let fields: Vec<Object<'py>> =
                        ["source_start", "source_end", "output_start", "output_end"]
                            .into_iter()
                            .map(|name| native_field(&span, name))
                            .collect::<PyResult<_>>()?;
                    rows.push(PyTuple::new(py, fields)?.into_any());
                }
                passes.push(PyTuple::new(py, rows)?.into_any());
            }
            native_field(expansion, "expanded_sql")?
        }
    };
    let request: Bound<'py, PyDict> = PyDict::new(py);
    request.set_item("authored", native_field(model, "authored_sql")?)?;
    request.set_item("query", authored_query)?;
    request.set_item("expanded", expanded)?;
    request.set_item("cleaned", cleaned)?;
    request.set_item("passes", passes)?;
    Ok(Bound::new(py, BindingPositions::new(request.extract()?)?)?.into_any())
}

/// How a relation's columns differ from what compiling assumed of them.
enum Drift {
    /// Every known column is there as it was, and these columns are new.
    Added(Vec<String>),
    /// A known column is missing, retyped or listed twice, or nothing was known.
    Changed,
}

/// How this dialect binds and stores columns, to compare listed columns with known ones.
struct Comparison {
    dialect: Option<String>,
    /// Whether names bind case-sensitively, compared by `bound_identity`.
    sensitive: bool,
    folds_upper: bool,
}

impl Comparison {
    fn identity(&self, name: &str) -> String {
        if self.sensitive {
            bound_identity(name, self.folds_upper)
        } else {
            name.to_owned()
        }
    }

    /// How `listed` columns differ from the `known` shape; `None` when they bind and store alike.
    fn drift(
        &self,
        listed: &[(String, Option<String>)],
        known: Option<&Object<'_>>,
    ) -> PyResult<Option<Drift>> {
        let Some(known) = known else {
            return Ok(Some(Drift::Changed));
        };
        let known: HashMap<String, String> = mapping_items(known)?
            .try_iter()?
            .map(|item| {
                let (name, column_type): (String, String) = item?.extract()?;
                Ok((self.identity(&name), column_type))
            })
            .collect::<PyResult<_>>()?;
        let mut seen: HashSet<String> = HashSet::new();
        let mut added: Vec<String> = Vec::new();
        for (name, listed_type) in listed {
            let identity: String = self.identity(name);
            if !seen.insert(identity.clone()) {
                return Ok(Some(Drift::Changed));
            }
            match known.get(&identity) {
                None => added.push(identity),
                Some(known_type) => {
                    let listed_type: &str = listed_type.as_deref().unwrap_or_default();
                    if !listed_type.is_empty()
                        && !same_column_type(self.dialect.as_deref(), known_type, listed_type)
                    {
                        return Ok(Some(Drift::Changed));
                    }
                }
            }
        }
        if known.keys().any(|name| !seen.contains(name)) {
            return Ok(Some(Drift::Changed));
        }
        Ok((!added.is_empty()).then_some(Drift::Added(added)))
    }

    /// Whether `model`'s result could change once the `drifted` relations gain `added` columns.
    fn reads_added(
        &self,
        model: &Object<'_>,
        drifted: &[String],
        added: &[String],
    ) -> PyResult<bool> {
        let Some(dialect) = self.dialect.as_deref() else {
            return Ok(true);
        };
        let query_sql: String = native_field(model, "query_sql")?.extract()?;
        let placeholders: Object<'_> = placeholders(model)?;
        let placeholders: Vec<(String, String)> = if placeholders.is_none() {
            Vec::new()
        } else {
            mapping_items(&placeholders)?
                .try_iter()?
                .map(|item| item?.extract())
                .collect::<PyResult<_>>()?
        };
        Ok(reads_added_columns(
            dialect,
            (&query_sql, &placeholders),
            drifted,
            added,
        ))
    }
}

/// `{str(key): str(value)}` of a dict `placeholders` config, else None.
fn placeholders<'py>(model: &Object<'py>) -> PyResult<Object<'py>> {
    let py: Python<'py> = model.py();
    let values: Object<'py> = native_field(&native_field(model, "config")?, "values")?
        .call_method1("get", ("placeholders",))?;
    if !values.is_instance_of::<PyDict>() {
        return Ok(py.None().into_bound(py));
    }
    let placeholders: Bound<'py, PyDict> = PyDict::new(py);
    for item in mapping_items(&values)?.try_iter()? {
        let (key, value): (Object<'py>, Object<'py>) = item?.extract()?;
        placeholders.set_item(key.str()?, value.str()?)?;
    }
    Ok(placeholders.into_any())
}

/// The column a name binds to on a case-sensitive dialect: quoted exactly, unquoted case-folded.
fn bound_identity(name: &str, folds_upper: bool) -> String {
    match name
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .filter(|inner| !inner.is_empty())
    {
        Some(inner) => inner.replace("\"\"", "\""),
        None if folds_upper => name.to_uppercase(),
        None => name.to_lowercase(),
    }
}

/// Whether a listed column is a plain identifier in one case, so it binds as an unquoted name.
fn is_folded_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
        && (!name.chars().any(|c| c.is_ascii_uppercase())
            || !name.chars().any(|c| c.is_ascii_lowercase()))
}

fn ignores_quoted_case<'py>(project: &Object<'py>, dialect: &Object<'py>) -> PyResult<Object<'py>> {
    let ignores: bool = crate::analysis::main::ignores_quoted_case::ignores_quoted_case(
        &native_field(project, "effective_connection")?,
        dialect,
    )?;
    Ok(PyBool::new(project.py(), ignores).to_owned().into_any())
}

fn replaced<'py>(value: &Object<'py>, pairs: &[(&str, Object<'py>)]) -> PyResult<Object<'py>> {
    let py: Python<'py> = value.py();
    let arguments: Bound<'py, PyDict> = PyDict::new(py);
    for (name, item) in pairs {
        arguments.set_item(*name, item)?;
    }
    replaced_keywords(value, &arguments)
}

fn call<'py>(callable: &Object<'py>, pairs: &[(&str, Object<'py>)]) -> PyResult<Object<'py>> {
    let arguments: Bound<'py, PyDict> = PyDict::new(callable.py());
    for (name, value) in pairs {
        arguments.set_item(*name, value)?;
    }
    callable.call((), Some(&arguments))
}
