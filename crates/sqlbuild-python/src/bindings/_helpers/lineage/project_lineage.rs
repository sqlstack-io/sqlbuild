//! Project column lineage in FAST or RICH mode, and direct semantic uses, from a project view.

use std::collections::HashMap;

use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyRef, PyResult, Python,
};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use pyo3::{pyfunction, wrap_pyfunction};
use sqlbuild_analysis::analysis_route::main::analysis_route::analysis_route;
use sqlbuild_analysis::analysis_route::models::AnalysisRoute;
use sqlbuild_analysis::lineage::main::build_fast_lineage::build_fast_lineage;
use sqlbuild_analysis::lineage::main::build_rich_lineage::build_rich_lineage;
use sqlbuild_analysis::lineage::main::direct_semantic_uses::direct_semantic_uses;
use sqlbuild_analysis::lineage::models::{
    DirectSemanticUse, FastLineageModel, FastLineageOutcome, FastLineageRequest, LineageColumn,
    RichLineageOutcome, RichLineageRequest,
};
use sqlbuild_analysis::lineage_graph::main::analysis_dialect::analysis_dialect;
use sqlbuild_analysis::lineage_graph::models::AnalysisDialect;
use sqlbuild_analysis::semantic_validation::models as validation;

use crate::bindings::_helpers::boundary::panics::compiler_error;
use crate::bindings::_helpers::compiled_views::compiled_model_view::NativeCompiledModel;
use crate::bindings::_helpers::compiled_views::model_reads::star_flags;
use crate::bindings::_helpers::lineage::column_lineage_graph::{
    CompactColumns, CompactModel, ProjectColumnLineage,
};
use crate::bindings::_helpers::lineage::lineage_objects::{LineageClasses, log_lineage_event};
use crate::bindings::_helpers::lineage::project_schema::{
    fast_schema, inferred_names, rich_schema,
};
use crate::bindings::_helpers::records::construct::error_class;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::ErrorId;
use crate::bindings::models::ProjectCatalog;
use crate::bindings::types::CompilerDetach;

const FAST_MODE: &str = "fast";
const RICH_MODE: &str = "rich";
const GENERIC_DIALECT: &str = "generic";

/// The project's column lineage graph in `mode`, or `None` without SQL analysis.
#[pyfunction]
pub(crate) fn build_project_column_lineage<'py>(
    project: &Bound<'py, PyAny>,
    dialect: Option<&Bound<'py, PyAny>>,
    mode: &Bound<'py, PyAny>,
    model_names: Option<&Bound<'py, PyAny>>,
) -> PyResult<Option<Py<PyAny>>> {
    let mode: String = mode.str()?.extract()?;
    if mode != FAST_MODE && mode != RICH_MODE {
        return Ok(None);
    }
    let classes: LineageClasses<'py> = LineageClasses::new(project.py())?;
    if !native_field(project, "settings")?
        .getattr("sql_analysis")?
        .is_truthy()?
    {
        return Ok(None);
    }
    let models: Vec<Bound<'py, PyAny>> = selected_models(project, model_names)?;
    let lineage: Bound<'py, PyAny> = if mode == FAST_MODE {
        fast_lineage(&classes, project, dialect, &models)?
    } else {
        rich_lineage(&classes, project, dialect, &models)?
    };
    Ok(Some(lineage.unbind()))
}

/// Direct non-projection semantic column uses of the requested models.
#[pyfunction]
pub(crate) fn build_direct_semantic_uses<'py>(
    project: &Bound<'py, PyAny>,
    dialect: Option<&str>,
    model_names: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyTuple>> {
    let py: Python<'py> = project.py();
    let classes: LineageClasses<'py> = LineageClasses::new(py)?;
    let schema = rich_schema(project)?;
    let mut models: Vec<(String, String, String)> = Vec::new();
    for model in native_field(project, "models")?.try_iter()? {
        let model: Bound<'py, PyAny> = model?;
        let name: Bound<'py, PyAny> = native_field(&model, "name")?;
        if model_names.contains(&name)? {
            models.push((
                name.extract()?,
                native_field(&model, "query_sql")?.extract()?,
                native_field(&model, "authored_sql")?.extract()?,
            ));
        }
    }
    let dialect: &str = dialect
        .filter(|name| !name.is_empty())
        .unwrap_or(GENERIC_DIALECT);
    let uses: Vec<DirectSemanticUse> = py
        .compiler_detach(|| direct_semantic_uses(&schema, &models, dialect))
        .map_err(compiler_error)?;
    let objects: Vec<Bound<'py, PyAny>> = uses
        .into_iter()
        .map(|usage| semantic_use(&classes, usage))
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, objects)
}

fn semantic_use<'py>(
    classes: &LineageClasses<'py>,
    usage: DirectSemanticUse,
) -> PyResult<Bound<'py, PyAny>> {
    let source: Bound<'py, PyAny> = classes.qualified(
        &classes
            .resource_type
            .call1((usage.resource_type.as_str(),))?,
        &usage.resource_name,
        &usage.column,
    )?;
    classes.semantic_use.call1((
        usage.model,
        usage.context,
        usage.expression,
        source,
        classes.confidence.call1((usage.confidence,))?,
        usage.line,
        usage.offset,
    ))
}

fn selected_models<'py>(
    project: &Bound<'py, PyAny>,
    model_names: Option<&Bound<'py, PyAny>>,
) -> PyResult<Vec<Bound<'py, PyAny>>> {
    let mut models: Vec<Bound<'py, PyAny>> = Vec::new();
    for model in native_field(project, "models")?.try_iter()? {
        let model: Bound<'py, PyAny> = model?;
        let selected: bool = match model_names {
            None => true,
            Some(names) => names.contains(native_field(&model, "name")?)?,
        };
        if selected {
            models.push(model);
        }
    }
    Ok(models)
}

/// One model's FAST lineage work: its compact facts and whether its root star expands.
struct FastModel<'py> {
    name: String,
    facts: FastFacts<'py>,
    has_star: bool,
    expands_star: bool,
}

/// A model's lineage facts: none, Python facts, or a compiled model's native facts and edge count.
enum FastFacts<'py> {
    Missing,
    /// No facts, on a native route, where Polyglot never parses the model: no lineage.
    Skipped,
    Python(Bound<'py, PyAny>),
    Native(Bound<'py, PyAny>, usize),
}

fn fast_lineage<'py>(
    classes: &LineageClasses<'py>,
    project: &Bound<'py, PyAny>,
    dialect: Option<&Bound<'py, PyAny>>,
    models: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = classes.py();
    let mut work: Vec<FastModel<'py>> = Vec::with_capacity(models.len());
    let mut requests: Vec<FastLineageModel> = Vec::new();
    let dialect_name: Option<String> = dialect.map(|name| name.extract()).transpose()?;
    let native: bool = !matches!(
        analysis_route(dialect_name.as_deref()),
        AnalysisRoute::Legacy
    );
    for model in models {
        let (has_star, star_resolved): (bool, bool) = star_flags(model)?;
        let expands_star: bool = has_star && !star_resolved;
        let native_edges: Option<usize> = match model.downcast::<NativeCompiledModel>() {
            Ok(view) if !expands_star => view.get().lineage_edge_count(py)?,
            _ => None,
        };
        if let Some(edges) = native_edges {
            work.push(FastModel {
                name: native_field(model, "name")?.extract()?,
                facts: FastFacts::Native(model.clone(), edges),
                has_star,
                expands_star,
            });
            continue;
        }
        let facts: Bound<'py, PyAny> = native_field(model, "fast_lineage_columns")?;
        if facts.is_none() && native {
            work.push(FastModel {
                name: native_field(model, "name")?.extract()?,
                facts: FastFacts::Skipped,
                has_star,
                expands_star,
            });
            continue;
        }
        if facts.is_none() {
            requests.push(FastLineageModel::Parse {
                query_sql: native_field(model, "query_sql")?.extract()?,
                inferred_columns: inferred_names(model)?,
            });
        } else if expands_star {
            requests.push(FastLineageModel::StarExpansion {
                query_sql: native_field(model, "query_sql")?.extract()?,
                existing_columns: output_columns(classes, &facts)?,
            });
        }
        work.push(FastModel {
            name: native_field(model, "name")?.extract()?,
            facts: if facts.is_none() {
                FastFacts::Missing
            } else {
                FastFacts::Python(facts)
            },
            has_star,
            expands_star,
        });
    }
    let outcomes: Vec<FastLineageOutcome> = if requests.is_empty() {
        Vec::new()
    } else {
        let binding_catalog: Bound<'py, PyAny> = native_field(project, "binding_catalog")?;
        let catalog: Option<PyRef<'py, ProjectCatalog>> = if binding_catalog.is_none() {
            None
        } else {
            native_field(&binding_catalog, "native")?.extract()?
        };
        let request = FastLineageRequest {
            dialect: dialect_name,
            schema: fast_schema(project)?,
            models: requests,
        };
        let native: Option<&validation::ProjectCatalog> =
            catalog.as_ref().map(|catalog| &catalog.inner);
        py.compiler_detach(|| build_fast_lineage(&request, native))
            .map_err(compiler_error)?
    };
    let mut outcomes = outcomes.into_iter();
    let built: Bound<'py, PyDict> = PyDict::new(py);
    let mut compact: HashMap<String, CompactModel> = HashMap::new();
    let mut model_order: Vec<String> = Vec::new();
    for model in work {
        let columns: CompactColumns = match model.facts {
            FastFacts::Native(view, edges) => CompactColumns::Model {
                model: view.unbind(),
                edges,
            },
            FastFacts::Python(mut columns) => {
                if model.expands_star {
                    let star_columns: Vec<LineageColumn> = outcomes
                        .next()
                        .map(|outcome| outcome.into_parts().columns)
                        .unwrap_or_default();
                    let mut expanded: Vec<Bound<'py, PyAny>> =
                        columns.try_iter()?.collect::<PyResult<_>>()?;
                    for column in &star_columns {
                        expanded.push(classes.column_fact(column)?);
                    }
                    columns = PyTuple::new(py, expanded)?.into_any();
                }
                CompactColumns::Facts(columns.unbind())
            }
            FastFacts::Skipped => continue,
            FastFacts::Missing => {
                match outcomes.next() {
                    Some(FastLineageOutcome::Built { columns, has_star }) => {
                        let lineage_columns: Vec<Bound<'py, PyAny>> = columns
                            .iter()
                            .map(|column| classes.column(column, &classes.unknown_nullability))
                            .collect::<PyResult<_>>()?;
                        built.set_item(
                            &model.name,
                            classes.model(&model.name, lineage_columns, has_star)?,
                        )?;
                        model_order.push(model.name);
                    }
                    Some(FastLineageOutcome::Unparsed(detail)) => log_lineage_event(
                        py,
                        "fast column lineage parse failed; falling back",
                        &model.name,
                        Some(&detail),
                    )?,
                    Some(FastLineageOutcome::UnknownDialect(detail)) => {
                        return Err(unknown_dialect_error(py, &detail));
                    }
                    _ => {}
                }
                continue;
            }
        };
        let _ = compact.insert(
            model.name.clone(),
            CompactModel {
                columns,
                has_star: model.has_star,
            },
        );
        model_order.push(model.name);
    }
    Bound::new(
        py,
        ProjectColumnLineage::compacted(classes, built.as_any(), compact, model_order)?,
    )
    .map(Bound::into_any)
}

/// The output column names of compact facts or plain column facts.
fn output_columns(classes: &LineageClasses<'_>, facts: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    if facts.is_instance(&classes.compact_facts)? {
        let pool: Bound<'_, PyAny> = native_field(facts, "string_pool")?;
        return native_field(facts, "rows")?
            .try_iter()?
            .map(|row| pool.get_item(row?.get_item(0)?)?.extract())
            .collect();
    }
    facts
        .try_iter()?
        .map(|column| native_field(&column?, "output_column")?.extract())
        .collect()
}

fn unknown_dialect_error(py: Python<'_>, dialect: &str) -> PyErr {
    match error_class(py, ErrorId::UnknownLineageDialect)
        .and_then(|class| class.call1((format!("Unknown dialect: {dialect}"),)))
    {
        Ok(error) => PyErr::from_value(error),
        Err(error) => error,
    }
}

fn rich_lineage<'py>(
    classes: &LineageClasses<'py>,
    project: &Bound<'py, PyAny>,
    dialect: Option<&Bound<'py, PyAny>>,
    models: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = classes.py();
    let outcomes: Vec<RichLineageOutcome> = if models.is_empty() {
        Vec::new()
    } else {
        let request = RichLineageRequest {
            dialect: polyglot_dialect(classes, dialect)?,
            schema: rich_schema(project)?,
            models: models
                .iter()
                .map(|model| native_field(model, "query_sql")?.extract())
                .collect::<PyResult<_>>()?,
        };
        py.compiler_detach(|| build_rich_lineage(&request))
            .map_err(compiler_error)?
    };
    let built: Bound<'py, PyDict> = PyDict::new(py);
    let mut edges: Vec<Bound<'py, PyAny>> = Vec::new();
    for (model, outcome) in models.iter().zip(outcomes) {
        let name: String = native_field(model, "name")?.extract()?;
        match outcome {
            RichLineageOutcome::Built { columns, has_star } => {
                let mut lineage_columns: Vec<Bound<'py, PyAny>> = Vec::with_capacity(columns.len());
                for column in &columns {
                    let nullability: Bound<'py, PyAny> =
                        classes.nullability.call1((column.nullability.as_str(),))?;
                    lineage_columns.push(classes.column(&column.column, &nullability)?);
                }
                built.set_item(&name, classes.model(&name, lineage_columns, has_star)?)?;
            }
            RichLineageOutcome::Skipped(detail) => log_lineage_event(
                py,
                "rich column lineage analysis failed; skipping model",
                &name,
                Some(&detail),
            )?,
        }
    }
    for item in built.iter() {
        let (target_name, result): (Bound<'py, PyAny>, Bound<'py, PyAny>) = item;
        for column in native_field(&result, "columns")?.try_iter()? {
            let column: Bound<'py, PyAny> = column?;
            let target: Bound<'py, PyAny> = classes.qualified_column.call1((
                &classes.model_resource,
                &target_name,
                native_field(&column, "output_column")?,
            ))?;
            let transform: Bound<'py, PyAny> = native_field(&column, "transform_kind")?;
            let confidence: Bound<'py, PyAny> = native_field(&column, "confidence")?;
            for upstream in native_field(&column, "upstream_columns")?.try_iter()? {
                let upstream: Bound<'py, PyAny> = upstream?;
                let source: Bound<'py, PyAny> = classes.qualified_column.call1((
                    native_field(&upstream, "resource_type")?,
                    native_field(&upstream, "resource_name")?,
                    native_field(&upstream, "column_name")?,
                ))?;
                edges.push(classes.edge(source, &target, &transform, &confidence)?);
            }
        }
    }
    let lineage: Bound<'py, PyAny> = py.get_type::<ProjectColumnLineage>().into_any();
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    fields.set_item("models", built)?;
    fields.set_item("edges", PyTuple::new(py, edges)?)?;
    lineage.call((), Some(&fields))
}

/// Python's `_polyglot_dialect(dialect).value`.
fn polyglot_dialect(
    classes: &LineageClasses<'_>,
    dialect: Option<&Bound<'_, PyAny>>,
) -> PyResult<String> {
    let Some(dialect) = dialect else {
        return Ok(GENERIC_DIALECT.to_owned());
    };
    let normalized: String = dialect
        .call_method0("strip")?
        .call_method0("lower")?
        .extract()?;
    match analysis_dialect(&normalized) {
        AnalysisDialect::Alias(value) => Ok(value.to_owned()),
        AnalysisDialect::Named(name) => {
            native_field(&classes.analysis_dialect.call1((name,))?, "value")?.extract()
        }
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(build_project_column_lineage, module)?)?;
    module.add_function(wrap_pyfunction!(build_direct_semantic_uses, module)?)?;
    Ok(())
}
