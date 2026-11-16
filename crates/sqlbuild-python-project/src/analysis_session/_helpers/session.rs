//! The native model analysis session and expression-source shapes.

use crate::analysis_session::models::FamilyRow;
use crate::analysis_session::models::ModelRow;
use crate::analysis_session::models::ProofRow;
use crate::analysis_session::models::RequestRow;
use pyo3::prelude::{Py, PyAny, PyErr, PyRef, PyResult, Python};
use pyo3::pyfunction;
use pyo3::types::PyDict;
use sqlbuild_analysis::assembly::analysis_session::main::expression_shapes::expression_shapes;
use sqlbuild_analysis::assembly::analysis_session::main::model_reference_facts::model_reference_facts;
use sqlbuild_analysis::assembly::analysis_session::main::names_set_operation::names_set_operation;
use sqlbuild_analysis::assembly::analysis_session::models::{
    ColumnFact, DynamicFamily, ExpressionShape, ExpressionShapeRequest, LineageFacts, LineageRow,
    ModelOutcome, ModelRequest, PivotModel, PivotOutcome, SessionRequest, SessionStep,
};
use sqlbuild_analysis::assembly::analysis_session::types::{Pairs, Shapes};
use sqlbuild_analysis::semantic_validation::models::DiagnosticRow;
use sqlbuild_python_base::positional_record;
use sqlbuild_sqltext::compiler::main::placeholder_defaults::placeholder_defaults;

use sqlbuild_core::constants::PANIC_MESSAGE;
use sqlbuild_core::panics::main::is_compiler_panic::is_compiler_panic;
use sqlbuild_core::panics::main::native_failure::native_failure;

use crate::analysis::models::Positional;
use crate::analysis::models::ProjectCatalog;
use crate::analysis_session::_helpers::nullability_rules::{
    RuleFailure, nullability_callback, raised,
};
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::types::CompilerDetach;

/// `(name, type, nullability)`.
pub(crate) type ColumnRow = (String, Option<String>, String);
positional_record! {
    into;
    /// One lineage column: output column, transform and confidence codes and its sources.
    pub(crate) struct LineageItem {
        output_column: String,
        transform_code: u8,
        confidence_code: u8,
        /// `(resource type, resource, column)` of each source.
        sources: Vec<(String, String, String)>,
    }
}

/// `(publications, failures)`.
pub(crate) type StepRow = (Shapes, Vec<String>);

positional_record! {
    into;
    /// One model's analysis, lineage source and cleaned SQL.
    pub(crate) struct OutcomeRow {
        pub(crate) analysis_succeeded: bool,
        pub(crate) columns: Option<Vec<ColumnRow>>,
        pub(crate) lineage_source: &'static str,
        pub(crate) lineage: Vec<LineageItem>,
        pub(crate) has_star: bool,
        pub(crate) star_resolved: bool,
        pub(crate) binding_diagnostics: Vec<Positional<DiagnosticRow>>,
        pub(crate) binding_validated: bool,
        pub(crate) cleaned_sql: String,
    }
}

/// One model's pivot SQL and declared families.
pub(crate) type PivotModelRow = (String, Vec<FamilyRow>);

/// `("absent" | "deferred" | "proof", proof)`.
pub(crate) type ContractRow = (&'static str, Option<ProofRow>);

positional_record! {
    extract;
    /// The settings and expressions one expression-source shape request infers.
    pub(crate) struct ExpressionShapesRow {
        pub(crate) dialect: String,
        pub(crate) case_sensitive_shapes: bool,
        pub(crate) function_return_types: Pairs,
        pub(crate) nullability_rules: Option<Pairs>,
        pub(crate) expressions: Vec<String>,
    }
}
/// `(catalog schema additions, analysis-shape names, whether each model's analysis succeeded)`.
pub(crate) type FinishRow = (Shapes, Vec<String>, Vec<bool>);

/// What an internal native failure of the session names.
const SESSION_CONTEXT: &str = "native model analysis";
const SHAPES_CONTEXT: &str = "native expression-source shapes";
/// The native store kind holding finished model analyses.
pub(crate) const MODEL_ANALYSIS_STORE_KIND: &str = "model-analyses";

/// An internal native failure of the session as `NativeCompilerError`.
pub(crate) fn internal_error(reason: &str) -> PyErr {
    if is_compiler_panic(reason) && reason != PANIC_MESSAGE {
        return compiler_error(reason);
    }
    compiler_error(native_failure(SESSION_CONTEXT, reason))
}

/// Each expression's shape, `None` where it has none; adapter rules run as in the session.
#[pyfunction]
#[pyo3(signature = (catalog, request, adapter_rules=None))]
pub(crate) fn infer_expression_source_shapes(
    py: Python<'_>,
    catalog: PyRef<'_, ProjectCatalog>,
    request: ExpressionShapesRow,
    adapter_rules: Option<(Py<PyDict>, Py<PyAny>)>,
) -> PyResult<Vec<Option<Pairs>>> {
    let ExpressionShapesRow {
        dialect,
        case_sensitive_shapes,
        function_return_types,
        nullability_rules,
        expressions,
    } = request;
    let rule_failure: RuleFailure = RuleFailure::default();
    let request = ExpressionShapeRequest {
        dialect,
        case_sensitive_shapes,
        function_return_types,
        nullability_rules,
        nullability_callback: adapter_rules.map(|(rules, nullability)| {
            nullability_callback(rules, nullability, rule_failure.clone())
        }),
        expressions,
    };
    let catalog = &catalog.inner;
    let shapes = py.compiler_detach(|| expression_shapes(catalog, &request));
    if let Some(error) = raised(&rule_failure) {
        return Err(error);
    }
    Ok(shapes
        .map_err(|reason| compiler_error(native_failure(SHAPES_CONTEXT, &reason)))?
        .into_iter()
        .map(|shape| match shape {
            ExpressionShape::Inferred(pairs) => Some(pairs),
            ExpressionShape::Absent => None,
        })
        .collect())
}

pub(crate) fn session_request(request: RequestRow) -> SessionRequest {
    let RequestRow {
        dialect,
        case_sensitive_shapes,
        quoted_ignore_case,
        function_return_types,
        known_functions,
        nullability_rules,
        rich_type_inference,
        column_types,
        column_nullability,
        complete_schemas,
        catalog_schemas,
        dynamic_families_by_table,
        models,
    } = request;
    SessionRequest {
        dialect,
        case_sensitive_shapes,
        quoted_ignore_case,
        function_return_types,
        known_functions,
        nullability_rules,
        nullability_callback: None,
        rich_type_inference,
        column_types,
        column_nullability,
        complete_schemas,
        catalog_schemas,
        dynamic_families_by_table: families_by_table(dynamic_families_by_table),
        models: models.into_iter().map(model_request).collect(),
        forced_order: None,
        retain_relations: sqlbuild_python_base::compile_context::main::delta::capturing::capturing(
        ),
    }
}

fn model_request(row: ModelRow) -> ModelRequest {
    let ModelRow {
        name,
        query_sql,
        placeholders,
        reference_rows,
        recover_cte_facts,
        snapshot_columns,
        dynamic_families,
        sql_macro_checks,
    } = row;
    let has_set_operation = names_set_operation(&query_sql);
    let facts = model_reference_facts(&reference_rows);
    let pivot_sql = if placeholders.is_empty() {
        query_sql.clone()
    } else {
        placeholder_defaults(&query_sql, &placeholders)
    };
    ModelRequest {
        name,
        query_sql,
        placeholders,
        references: facts.references,
        lineage_references: facts.lineage_references,
        required_names: facts.required_names,
        recover_cte_facts,
        has_set_operation,
        snapshot_columns,
        pivot_sql,
        dynamic_families: dynamic_families.into_iter().map(dynamic_family).collect(),
        sql_macro_checks,
    }
}

pub(crate) fn step_row(step: SessionStep) -> StepRow {
    (step.publications, step.failures)
}

fn column_row(column: ColumnFact) -> ColumnRow {
    (column.name, column.data_type, column.nullability)
}

fn lineage_items(rows: Vec<LineageRow>) -> Vec<LineageItem> {
    rows.into_iter()
        .map(|row| LineageItem {
            output_column: row.output_column,
            transform_code: row.transform_code,
            confidence_code: row.confidence_code,
            sources: row.sources,
        })
        .collect()
}

pub(crate) fn families_by_table(
    rows: Vec<(String, Vec<FamilyRow>)>,
) -> Vec<(String, Vec<DynamicFamily>)> {
    rows.into_iter()
        .map(|(name, families)| (name, families.into_iter().map(dynamic_family).collect()))
        .collect()
}

fn dynamic_family(row: FamilyRow) -> DynamicFamily {
    let FamilyRow {
        name,
        pivot_column,
        value_column,
        aggregate,
        data_type,
        name_pattern,
    } = row;
    DynamicFamily {
        name,
        pivot_column,
        value_column,
        aggregate,
        data_type,
        name_pattern,
    }
}

pub(crate) fn contract_row(outcome: PivotOutcome) -> ContractRow {
    match outcome {
        PivotOutcome::Absent => ("absent", None),
        PivotOutcome::Proof(proof) => (
            "proof",
            Some(ProofRow {
                output_proven: proof.output_proven,
                fixed_columns: proof.fixed_columns.into_iter().map(column_row).collect(),
                families: proof.families,
                input_relations: proof.input_relations,
                failure_reason: proof.failure_reason,
                bare_dynamic_pivot: proof.bare_dynamic_pivot,
            }),
        ),
    }
}

pub(crate) fn outcome_row(outcome: ModelOutcome, cleaned_sql: String) -> OutcomeRow {
    let analysis = outcome.analysis;
    let (source, lineage) = match analysis.lineage {
        LineageFacts::Native(rows) => ("native", lineage_items(rows)),
        LineageFacts::NativeFacts(rows) => ("facts", lineage_items(rows)),
    };
    OutcomeRow {
        analysis_succeeded: analysis.analysis_succeeded,
        columns: analysis
            .columns
            .map(|columns| columns.into_iter().map(column_row).collect()),
        lineage_source: source,
        lineage,
        has_star: analysis.has_star,
        star_resolved: analysis.star_resolved,
        binding_diagnostics: Positional::wrap_all(analysis.binding_diagnostics),
        binding_validated: analysis.binding_validated,
        cleaned_sql,
    }
}

pub(crate) fn pivot_model(row: PivotModelRow) -> PivotModel {
    let (sql, families) = row;
    PivotModel {
        sql,
        families: families.into_iter().map(dynamic_family).collect(),
    }
}
