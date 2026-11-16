//! Project column lineage in FAST or RICH mode, and direct semantic uses, from a project view.

use sqlbuild_python_base::records::main::construct::new_plain_error::new_plain_error;
use std::collections::HashMap;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyRef, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use sqlbuild_analysis::analysis_route::main::analysis_route::analysis_route;
use sqlbuild_analysis::analysis_route::models::AnalysisRoute;
use sqlbuild_analysis::lineage::main::build_fast_lineage::build_fast_lineage;
use sqlbuild_analysis::lineage::main::build_rich_lineage::build_rich_lineage;
use sqlbuild_analysis::lineage::models::{
    DirectSemanticUse, FastLineageModel, FastLineageOutcome, FastLineageRequest, LineageColumn,
    RichLineageOutcome, RichLineageRequest,
};
use sqlbuild_analysis::lineage_graph::main::analysis_dialect::analysis_dialect;
use sqlbuild_analysis::lineage_graph::models::AnalysisDialect;
use sqlbuild_analysis::semantic_validation::models as validation;

use crate::lineage::_helpers::graph::project_schema::fast_schema;
use crate::lineage::_helpers::graph::project_schema::inferred_names;
use crate::lineage::_helpers::graph::project_schema::rich_schema;
use crate::lineage::_helpers::lineage::lineage_objects::log_lineage_event;
use crate::lineage::models::CompactColumns;
use crate::lineage::models::CompactModel;
use crate::lineage::models::LineageClasses;
use crate::lineage::models::ProjectColumnLineage;
use crate::project_assembly::main::model_placeholders::model_placeholders;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_base::types::CompilerDetach;
use sqlbuild_python_project::analysis::models::ProjectCatalog;
use sqlbuild_python_project::compiled_views::main::compiled_models::star_flags::star_flags;
use sqlbuild_python_project::compiled_views::models::NativeCompiledModel;

pub(crate) const FAST_MODE: &str = "fast";
pub(crate) const RICH_MODE: &str = "rich";
pub(crate) const GENERIC_DIALECT: &str = "generic";

pub(crate) fn semantic_use<'py>(
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

pub(crate) fn selected_models<'py>(
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
    Python(Bound<'py, PyAny>),
    Native(Bound<'py, PyAny>, usize),
}

pub(crate) fn fast_lineage<'py>(
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
        if facts.is_none() {
            requests.push(FastLineageModel::Parse {
                query_sql: native_field(model, "query_sql")?.extract()?,
                inferred_columns: inferred_names(model)?,
                placeholders: if native {
                    placeholder_pairs(model)?
                } else {
                    Vec::new()
                },
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
                        if native {
                            "fast column lineage not built: the native parser cannot read the model"
                        } else {
                            "fast column lineage parse failed; falling back"
                        },
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

/// The model's `@@@name` placeholder defaults, which a native parser reads in their place.
fn placeholder_pairs(model: &Bound<'_, PyAny>) -> PyResult<Vec<(String, String)>> {
    let Some(placeholders) = model_placeholders(model)? else {
        return Ok(Vec::new());
    };
    placeholders
        .iter()
        .map(|(name, value)| Ok((name.extract()?, value.extract()?)))
        .collect()
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
    new_plain_error(
        py,
        ErrorId::UnknownLineageDialect,
        &format!("Unknown dialect: {dialect}"),
    )
}

pub(crate) fn rich_lineage<'py>(
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
