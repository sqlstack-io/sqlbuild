//! Compare each built model with `--against`: schema, row counts, keys and values.

use std::collections::{BTreeMap, BTreeSet};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyTuple, PyTupleMethods};
use sqlbuild_analysis::review_command::constants::{
    BOUNDED_COMPARISON_MODE, EXACT_LIMIT, FULL_COMPARISON_MODE, MAX_FINDING_EXAMPLES,
    MISSING_AGAINST_CODE, NO_COMPARISON_MODE, SCHEMA_ONLY_MODE,
};
use sqlbuild_analysis::review_command::models::{
    ColumnEvidence, IncompleteCheck, NodeComparison, ReviewExample, ReviewFinding, ReviewNode,
};
use sqlbuild_cli::diff_report::models::{
    DiffColumnFacts, DiffModelFacts, DiffRowFacts, DiffSampleRow, DiffValue,
};
use sqlbuild_cli::diff_report::types::DiffNamedValue;
use sqlbuild_host::entries::models::ExecutorHost;

use crate::bindings::_helpers::compile_driver::diff::run_diff_pipeline;
use crate::bindings::_helpers::compiled_views::project_reads;
use crate::bindings::_helpers::dagster_invocation::access::string_tuple as texts;
use crate::bindings::_helpers::diff_report::command::target_max_full_rows;
use crate::bindings::_helpers::diff_report::facts::diff_facts;
use crate::bindings::_helpers::diff_report::support::{error_text, matches_class};
use crate::bindings::_helpers::execution_history::conversion::optional_text;
use crate::bindings::_helpers::execution_history::statements::fetch_one;
use crate::bindings::_helpers::review_command::planning::ReviewSettings;
use crate::bindings::_helpers::review_command::support::review_error;
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::build_command::main::boolean::boolean;
use sqlbuild_python_base::build_command::main::text::text;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const KEYED_CHECK: &str = "keyed comparison";
const MAX_EXAMPLE_LENGTH: usize = 80;

/// The comparison of every built model, and the findings and incomplete checks it produced.
#[derive(Default)]
pub(crate) struct Comparisons {
    pub(crate) by_node: BTreeMap<String, NodeComparison>,
    pub(crate) findings: Vec<ReviewFinding>,
    pub(crate) incomplete: Vec<IncompleteCheck>,
}

/// The two compiled projects and the connection a comparison runs on, borrowed for the run.
pub(crate) struct ComparisonContext<'a, 'py> {
    pub(crate) request: &'a Bound<'py, PyAny>,
    pub(crate) discovered: &'a Bound<'py, PyAny>,
    pub(crate) adapter: &'a Bound<'py, PyAny>,
    pub(crate) connection: &'a Bound<'py, PyAny>,
    pub(crate) connection_config: &'a Bound<'py, PyAny>,
    pub(crate) review_target: &'a str,
}

fn short(value: &DiffValue) -> String {
    let text: &str = &value.text;
    if text.chars().count() <= MAX_EXAMPLE_LENGTH {
        return text.to_owned();
    }
    let kept: String = text.chars().take(MAX_EXAMPLE_LENGTH).collect();
    format!("{kept}…")
}

fn key_text(values: &[(String, DiffValue)]) -> String {
    values
        .iter()
        .map(|(name, value)| format!("{name}={}", short(value)))
        .collect::<Vec<String>>()
        .join(", ")
}

/// The names of `project`'s models among `names`, each with its compiled model.
fn models_named<'py>(
    project: &Bound<'py, PyAny>,
    names: &[String],
) -> PyResult<Vec<(String, Bound<'py, PyAny>)>> {
    let mut models: Vec<(String, Bound<'py, PyAny>)> = Vec::new();
    for model in project_reads::models(project)?.try_iter()? {
        let model: Bound<'py, PyAny> = model?;
        let name: String = native_field(&model, "name")?.extract()?;
        if names.contains(&name) {
            models.push((name, model));
        }
    }
    Ok(models)
}

/// Whether the target's relation of `model` exists.
fn target_exists(context: &ComparisonContext<'_, '_>, model: &Bound<'_, PyAny>) -> PyResult<bool> {
    let destination: Bound<'_, PyAny> = native_field(model, "destination")?;
    call_with(
        &context.adapter.getattr("relation_exists")?,
        &[
            ("connection", context.connection.clone()),
            ("database", native_field(&destination, "database")?),
            ("schema", native_field(&destination, "schema")?),
            ("name", native_field(&destination, "name")?),
        ],
    )?
    .is_truthy()
}

fn row_count(context: &ComparisonContext<'_, '_>, relation: &str) -> PyResult<Option<i64>> {
    let cursor: Bound<'_, PyAny> = call_with(
        &context.adapter.getattr("execute")?,
        &[
            ("connection", context.connection.clone()),
            (
                "sql",
                text(
                    context.adapter.py(),
                    &format!("SELECT COUNT(*) FROM {relation}"),
                ),
            ),
        ],
    )?;
    let Some(row) = fetch_one(&cursor)? else {
        return Ok(None);
    };
    row.get_item(0)?.extract()
}

/// One `execute_diff` call for `name`.
fn run_one<'py>(
    context: &ComparisonContext<'_, 'py>,
    (left, right): (&Bound<'py, PyAny>, &Bound<'py, PyAny>),
    name: &str,
    options: &[(&str, Bound<'py, PyAny>)],
) -> PyResult<Option<DiffModelFacts>> {
    let py: Python<'py> = context.adapter.py();
    let mut pairs: Vec<(&str, Bound<'py, PyAny>)> = vec![
        ("collect_samples", boolean(py, true)),
        (
            "max_column_examples",
            pyo3::IntoPyObjectExt::into_bound_py_any(MAX_FINDING_EXAMPLES, py)?,
        ),
        (
            "max_row_only_examples",
            pyo3::IntoPyObjectExt::into_bound_py_any(MAX_FINDING_EXAMPLES, py)?,
        ),
    ];
    pairs.extend(options.iter().cloned());
    let options: Bound<'py, PyAny> =
        call_with(&host_entry(py, ExecutorHost::DiffExecutionOptions)?, &pairs)?;
    let result: Bound<'py, PyAny> = call_with(
        &host_entry(py, ExecutorHost::ExecuteDiff)?,
        &[
            ("adapter", context.adapter.clone()),
            ("connection", context.connection.clone()),
            ("left_project", left.clone()),
            ("right_project", right.clone()),
            ("selected_names", PyTuple::new(py, [name])?.into_any()),
            ("options", options),
        ],
    )?;
    Ok(diff_facts(&result)?.into_iter().next())
}

fn incomplete(node: &str, reason: String) -> IncompleteCheck {
    IncompleteCheck {
        node: node.to_owned(),
        check: KEYED_CHECK.to_owned(),
        reason,
    }
}

/// One model's comparison, with the findings and incomplete checks it produced.
#[derive(Default)]
struct ModelComparison {
    comparison: Option<NodeComparison>,
    findings: Vec<ReviewFinding>,
    incomplete: Vec<IncompleteCheck>,
}

fn row_count_finding(node: &str, (target, branch): (i64, i64)) -> ReviewFinding {
    ReviewFinding {
        node: node.to_owned(),
        kind: "row_count".to_owned(),
        metric: "row count".to_owned(),
        target_value: Some(target.to_string()),
        branch_value: Some(branch.to_string()),
        limit: EXACT_LIMIT.to_owned(),
        ..ReviewFinding::default()
    }
}

fn schema_finding(facts: &DiffModelFacts) -> Option<ReviewFinding> {
    let mut parts: Vec<String> = Vec::new();
    if !facts.added_columns.is_empty() {
        parts.push(format!("added {}", facts.added_columns.join(", ")));
    }
    if !facts.removed_columns.is_empty() {
        parts.push(format!("removed {}", facts.removed_columns.join(", ")));
    }
    for (column, from, to) in &facts.type_changes {
        parts.push(format!("{column} {from} → {to}"));
    }
    (!parts.is_empty()).then(|| ReviewFinding {
        node: facts.name.clone(),
        kind: "schema".to_owned(),
        metric: format!("schema changed: {}", parts.join("; ")),
        limit: EXACT_LIMIT.to_owned(),
        ..ReviewFinding::default()
    })
}

/// The example of one sample row whose `column` changed, if it did.
fn cell_example(sample: &DiffSampleRow, column: &str) -> Option<ReviewExample> {
    let cell = sample
        .changed_cells
        .iter()
        .find(|cell| cell.name == column)?;
    Some(ReviewExample {
        key: key_text(&sample.key_values),
        target: Some(short(&cell.left)),
        branch: Some(short(&cell.right)),
    })
}

fn key_examples(keys: &[Vec<DiffNamedValue>]) -> Vec<ReviewExample> {
    keys.iter()
        .take(MAX_FINDING_EXAMPLES)
        .map(|key| ReviewExample {
            key: key_text(key),
            ..ReviewExample::default()
        })
        .collect()
}

fn key_findings(facts: &DiffModelFacts, rows: &DiffRowFacts) -> Vec<ReviewFinding> {
    [
        (rows.right_only_count, "branch-only", &facts.right_only_keys),
        (rows.left_only_count, "target-only", &facts.left_only_keys),
    ]
    .into_iter()
    .filter(|(count, _, _)| *count > 0)
    .map(|(count, side, keys)| ReviewFinding {
        node: facts.name.clone(),
        kind: "keys".to_owned(),
        metric: format!("{count} {side} keys"),
        limit: EXACT_LIMIT.to_owned(),
        examples: key_examples(keys),
        ..ReviewFinding::default()
    })
    .collect()
}

/// The comparison evidence and findings of one model's diff facts.
fn evidence(facts: &DiffModelFacts, mode: &str) -> (NodeComparison, Vec<ReviewFinding>) {
    let mut comparison: NodeComparison = NodeComparison {
        mode: mode.to_owned(),
        target_relation: Some(facts.left_relation.clone()),
        branch_relation: Some(facts.right_relation.clone()),
        unique_key: facts.unique_key.clone(),
        added_columns: facts.added_columns.clone(),
        removed_columns: facts.removed_columns.clone(),
        type_changes: facts.type_changes.clone(),
        complete: mode == FULL_COMPARISON_MODE,
        ..NodeComparison::default()
    };
    let mut findings: Vec<ReviewFinding> = schema_finding(facts).into_iter().collect();
    let Some(rows) = &facts.rows else {
        return (comparison, findings);
    };
    comparison.target_rows = Some(rows.left_count);
    comparison.branch_rows = Some(rows.right_count);
    comparison.compared_rows = Some(rows.compared_count);
    comparison.branch_only_keys = rows.right_only_count;
    comparison.target_only_keys = rows.left_only_count;
    if rows.left_count != rows.right_count {
        findings.push(row_count_finding(
            &facts.name,
            (rows.left_count, rows.right_count),
        ));
    }
    findings.extend(key_findings(facts, rows));
    let changed: Vec<&DiffColumnFacts> = rows
        .columns
        .iter()
        .filter(|column| column.mismatched_count > 0)
        .collect();
    for column in &changed {
        comparison.columns.push(ColumnEvidence {
            column: column.name.clone(),
            metric: "rows that differ".to_owned(),
            target: "0".to_owned(),
            branch: column.mismatched_count.to_string(),
            limit: "0".to_owned(),
        });
        let examples: Vec<ReviewExample> = facts
            .unequal_samples
            .iter()
            .filter_map(|sample| cell_example(sample, &column.name))
            .take(MAX_FINDING_EXAMPLES)
            .collect();
        findings.push(ReviewFinding {
            node: facts.name.clone(),
            kind: "values".to_owned(),
            metric: format!(
                "values differ in {} of {} matched rows",
                column.mismatched_count, rows.joined_count
            ),
            column: Some(column.name.clone()),
            limit: EXACT_LIMIT.to_owned(),
            examples,
            ..ReviewFinding::default()
        });
    }
    comparison.unchanged_columns = rows.columns.len().saturating_sub(changed.len());
    (comparison, findings)
}

/// Both compiled sides, the exact-comparison size limits, and the models `--against` holds.
pub(crate) struct Sides<'py> {
    left: Bound<'py, PyAny>,
    right: Bound<'py, PyAny>,
    limits: Bound<'py, PyAny>,
    bounded: Option<String>,
    against_has: BTreeSet<String>,
}

/// A model without a unique key: its schema and row counts, with the keyed check incomplete.
fn compare_unkeyed(
    context: &ComparisonContext<'_, '_>,
    sides: &Sides<'_>,
    name: &str,
) -> PyResult<ModelComparison> {
    let py: Python<'_> = context.adapter.py();
    let Some(facts) = run_one(
        context,
        (&sides.left, &sides.right),
        name,
        &[("schema_only", boolean(py, true))],
    )?
    else {
        return Ok(ModelComparison::default());
    };
    let (mut comparison, mut findings) = evidence(&facts, SCHEMA_ONLY_MODE);
    comparison.target_rows = row_count(context, &facts.left_relation)?;
    comparison.branch_rows = row_count(context, &facts.right_relation)?;
    if let (Some(target), Some(branch)) = (comparison.target_rows, comparison.branch_rows)
        && target != branch
    {
        findings.push(row_count_finding(name, (target, branch)));
    }
    Ok(ModelComparison {
        comparison: Some(comparison),
        findings,
        incomplete: vec![incomplete(
            name,
            "the model has no unique_key, so only the schema and row counts were compared"
                .to_owned(),
        )],
    })
}

/// A keyed model too large for an exact comparison: a bounded window, or the schema only.
fn compare_guarded<'py>(
    context: &ComparisonContext<'_, 'py>,
    sides: &Sides<'py>,
    node: &ReviewNode,
) -> PyResult<(Option<DiffModelFacts>, &'static str, IncompleteCheck)> {
    let py: Python<'py> = context.adapter.py();
    let name: &str = &node.name;
    if let (Some(window), true) = (&sides.bounded, node.has_cursor) {
        let facts = run_one(
            context,
            (&sides.left, &sides.right),
            name,
            &[
                ("schema_only", boolean(py, false)),
                ("bounded", text(py, window)),
            ],
        )?;
        let reason: String =
            format!("too large for an exact comparison; compared the last {window} only");
        return Ok((facts, BOUNDED_COMPARISON_MODE, incomplete(name, reason)));
    }
    let hint: &str = if node.has_cursor {
        "; pass --bounded <window> to compare a recent window"
    } else {
        ""
    };
    let facts = run_one(
        context,
        (&sides.left, &sides.right),
        name,
        &[("schema_only", boolean(py, true))],
    )?;
    let reason: String = format!("too large for an exact comparison, or its size is unknown{hint}");
    Ok((facts, SCHEMA_ONLY_MODE, incomplete(name, reason)))
}

/// A keyed model: an exact comparison when the size guard allows it.
fn compare_keyed<'py>(
    context: &ComparisonContext<'_, 'py>,
    sides: &Sides<'py>,
    node: &ReviewNode,
) -> PyResult<ModelComparison> {
    let py: Python<'py> = context.adapter.py();
    let name: &str = &node.name;
    let full = run_one(
        context,
        (&sides.left, &sides.right),
        name,
        &[
            ("schema_only", boolean(py, false)),
            ("full_size_limits", sides.limits.clone()),
        ],
    );
    let (facts, mode, checks) = match full {
        Ok(facts) => (facts, FULL_COMPARISON_MODE, Vec::new()),
        Err(error) if matches_class(py, &error, ExecutorHost::FullDiffSizeGuardError)? => {
            let (facts, mode, check) = compare_guarded(context, sides, node)?;
            (facts, mode, vec![check])
        }
        Err(error) if error.is_instance_of::<pyo3::exceptions::PyException>(py) => {
            return Ok(ModelComparison {
                comparison: Some(NodeComparison {
                    mode: NO_COMPARISON_MODE.to_owned(),
                    target_relation: Some(String::new()),
                    ..NodeComparison::default()
                }),
                findings: Vec::new(),
                incomplete: vec![incomplete(name, error_text(py, &error)?)],
            });
        }
        Err(error) => return Err(error),
    };
    let Some(facts) = facts else {
        return Ok(ModelComparison {
            incomplete: checks,
            ..ModelComparison::default()
        });
    };
    let (comparison, findings) = evidence(&facts, mode);
    Ok(ModelComparison {
        comparison: Some(comparison),
        findings,
        incomplete: checks,
    })
}

/// One model: keyed or unkeyed, or uncompared when `--against` lacks it, incomplete unless new.
fn compare_model(
    context: &ComparisonContext<'_, '_>,
    sides: &Sides<'_>,
    (node, new): (&ReviewNode, bool),
) -> PyResult<ModelComparison> {
    if !sides.against_has.contains(&node.name) {
        return Ok(ModelComparison {
            comparison: Some(NodeComparison {
                mode: NO_COMPARISON_MODE.to_owned(),
                complete: new,
                ..NodeComparison::default()
            }),
            incomplete: if new {
                Vec::new()
            } else {
                vec![incomplete(
                    &node.name,
                    "--against has no relation for this model, which existed at the base \
                     revision"
                        .to_owned(),
                )]
            },
            ..ModelComparison::default()
        });
    }
    let compared: ModelComparison = if node.unique_key.is_empty() {
        compare_unkeyed(context, sides, &node.name)?
    } else {
        compare_keyed(context, sides, node)?
    };
    if compared.comparison.is_none() && compared.incomplete.is_empty() {
        return Ok(ModelComparison {
            incomplete: vec![incomplete(
                &node.name,
                "the comparison returned no result".to_owned(),
            )],
            ..compared
        });
    }
    Ok(compared)
}

/// Compile both sides of `models`, failing when `--against` holds none that existed at the base.
pub(crate) fn prepare_sides<'py>(
    context: &ComparisonContext<'_, 'py>,
    settings: &ReviewSettings,
    (models, new): (&[String], &BTreeSet<String>),
) -> PyResult<Option<Sides<'py>>> {
    if models.is_empty() {
        return Ok(None);
    }
    let py: Python<'py> = context.adapter.py();
    let pipeline: Bound<'py, PyTuple> = run_diff_pipeline(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", context.discovered.clone()),
                ("adapter", context.adapter.clone()),
                ("from_target", text(py, &settings.against)),
                ("to_target", text(py, context.review_target)),
                ("resolved_connection", context.connection_config.clone()),
                ("no_sql_validation", boolean(py, true)),
                ("select", texts(py, models)?),
                ("exclude", PyTuple::empty(py).into_any()),
                ("cli_vars", native_field(context.request, "cli_vars")?),
            ],
        )?),
    )?;
    let left: Bound<'py, PyAny> = pipeline.get_item(0)?;
    let mut against_has: BTreeSet<String> = BTreeSet::new();
    for (name, model) in models_named(&left, models)? {
        if target_exists(context, &model)? {
            against_has.insert(name);
        }
    }
    let existing: Vec<&String> = models.iter().filter(|name| !new.contains(*name)).collect();
    if !existing.is_empty() && !existing.iter().any(|name| against_has.contains(*name)) {
        return Err(review_error(
            py,
            MISSING_AGAINST_CODE,
            &format!(
                "--against has none of these relations; check the target: {}",
                existing
                    .iter()
                    .map(|name| name.as_str())
                    .collect::<Vec<&str>>()
                    .join(", ")
            ),
            Some("pass the target that holds the deployed tables, such as --against prod"),
        ));
    }
    let limits: Bound<'py, PyAny> = call_with(
        &host_entry(py, ExecutorHost::FullDiffSizeLimits)?,
        &[
            ("left_target", text(py, &settings.against)),
            ("right_target", text(py, context.review_target)),
            (
                "left_max_rows",
                target_max_full_rows(context.discovered, &text(py, &settings.against))?,
            ),
            (
                "right_max_rows",
                target_max_full_rows(context.discovered, &text(py, context.review_target))?,
            ),
        ],
    )?;
    Ok(Some(Sides {
        right: pipeline.get_item(1)?,
        left,
        limits,
        bounded: optional_text(&native_field(context.request, "bounded")?)?,
        against_has,
    }))
}

/// Compare each of `models` (built, by name) between `--against` and the review schema.
pub(crate) fn compare_models(
    context: &ComparisonContext<'_, '_>,
    sides: Option<&Sides<'_>>,
    (models, new): (&[&ReviewNode], &BTreeSet<String>),
) -> PyResult<Comparisons> {
    let mut out: Comparisons = Comparisons::default();
    let Some(sides) = sides else {
        return Ok(out);
    };
    for node in models {
        let compared: ModelComparison =
            compare_model(context, sides, (node, new.contains(&node.name)))?;
        out.findings.extend(compared.findings);
        out.incomplete.extend(compared.incomplete);
        if let Some(comparison) = compared.comparison {
            out.by_node.insert(node.name.clone(), comparison);
        }
    }
    Ok(out)
}
