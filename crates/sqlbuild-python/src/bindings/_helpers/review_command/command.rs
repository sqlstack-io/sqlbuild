//! `sqb review`: plan the changed set, build it in a run-owned schema, test, compare and report.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Instant;

use pyo3::IntoPyObject;
use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use sqlbuild_analysis::review_command::constants::{
    AUDIT_FINDING, BUILT_NODE, FAILED_NODE, FINDING_NODE, MARKDOWN_WRITE_CODE, MODEL_KIND,
    NO_COMPARISON_MODE, NOT_VALIDATED_NODE, OK_NODE, PASSES_LIMIT, PLANNED_STATUS, SEED_KIND,
    TEST_FINDING, UNIT_TEST_LABEL,
};
use sqlbuild_analysis::review_command::main::downstream_policy_name::downstream_policy_name;
use sqlbuild_analysis::review_command::main::plan_review::plan_review;
use sqlbuild_analysis::review_command::main::render_review_json::render_review_json;
use sqlbuild_analysis::review_command::main::render_review_markdown::render_review_markdown;
use sqlbuild_analysis::review_command::main::render_review_text::render_review_text;
use sqlbuild_analysis::review_command::main::review_outcome::review_outcome;
use sqlbuild_analysis::review_command::models::{
    IncompleteCheck, NodeReport, ReviewFailure, ReviewFinding, ReviewNode, ReviewPlan,
    ReviewReport, ReviewSchemaReport, ValidationCounts,
};
use sqlbuild_core::git_revisions::main::head_commit::head_commit;

use crate::bindings::_helpers::clone::clone_progress::write_line;
use crate::bindings::_helpers::recall::console::{write_stderr, write_stdout_bytes};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::review_command::comparison::{
    ComparisonContext, Comparisons, Sides, compare_models, prepare_sides,
};
use crate::bindings::_helpers::review_command::execution::{
    BuildOutcome, RunInputs, build_validated, incremental_starts, run_changed_tests,
};
use crate::bindings::_helpers::review_command::impact::affected_columns;
use crate::bindings::_helpers::review_command::planning::{
    Compiled, ReviewProject, ReviewSettings, discover, review_project, review_settings,
    selected_tokens,
};
use crate::bindings::_helpers::review_command::schema::{
    ReviewSchema, connection_config, create_review_schema, drop_review_schema, review_schema,
    with_review_target,
};
use crate::bindings::_helpers::review_command::support::{
    error_parts, failed_error, invocation_id, phase, plan_error, progress_stream, utc_now_compact,
};

const SUCCESS_STATUS: &str = "success";
const SKIPPED_STATUS: &str = "skipped";
const REVIEW_WORK_DIRECTORY: [&str; 2] = ["target", "review"];
const BUILD_CHECK: &str = "build";
const COMPARISON_CHECK: &str = "comparison";
const SKIPPED_REASON: &str = "skipped after an upstream failure";
const UNCOMPARED_SEED_REASON: &str =
    "seeds are not compared: the diff pipeline compares models only";
const NEW_NODE_REASON: &str = "new in this branch; nothing in --against to compare";

/// The nodes the changed set reaches, with edges between them.
fn impact_edges(project: &ReviewProject, plan: &ReviewPlan) -> Vec<(String, String)> {
    let affected: BTreeSet<&str> = plan
        .changed
        .iter()
        .map(|node| node.name.as_str())
        .chain(plan.not_validated.iter().map(|node| node.name.as_str()))
        .chain(plan.validated.iter().map(String::as_str))
        .collect();
    let mut edges: Vec<(String, String)> = Vec::new();
    for (upstream, children) in &project.review_graph.downstream {
        if !affected.contains(upstream.as_str()) {
            continue;
        }
        for child in children {
            if affected.contains(child.as_str()) {
                edges.push((upstream.clone(), child.clone()));
            }
        }
    }
    edges
}

/// Upstream nodes of the validated set that the build reads from `--against`.
fn deferred(project: &ReviewProject, validated: &[String]) -> Vec<String> {
    let built: BTreeSet<&str> = validated.iter().map(String::as_str).collect();
    let mut upstream: BTreeSet<String> = BTreeSet::new();
    for (parent, children) in &project.review_graph.downstream {
        if !built.contains(parent.as_str())
            && children.iter().any(|child| built.contains(child.as_str()))
        {
            upstream.insert(parent.clone());
        }
    }
    project
        .review_graph
        .order
        .iter()
        .filter(|name| upstream.contains(*name))
        .cloned()
        .collect()
}

/// The impact tree's nodes before anything runs: planned, or not validated.
fn planned_nodes(project: &ReviewProject, plan: &ReviewPlan) -> Vec<NodeReport> {
    let changed: BTreeSet<&str> = plan.changed.iter().map(|node| node.name.as_str()).collect();
    let validated: BTreeSet<&str> = plan.validated.iter().map(String::as_str).collect();
    let left_out: BTreeMap<&str, &str> = plan
        .not_validated
        .iter()
        .map(|node| (node.name.as_str(), node.reason.as_str()))
        .collect();
    project
        .review_graph
        .order
        .iter()
        .filter(|name| validated.contains(name.as_str()) || left_out.contains_key(name.as_str()))
        .map(|name| {
            let node: &ReviewNode = &project.review_graph.nodes[name];
            let reason: Option<&str> = left_out.get(name.as_str()).copied();
            NodeReport {
                name: name.clone(),
                kind: node.kind.clone(),
                critical: node.critical,
                changed: changed.contains(name.as_str()),
                status: match reason {
                    Some(_) => NOT_VALIDATED_NODE.to_owned(),
                    None => PLANNED_STATUS.to_owned(),
                },
                reason: reason.map(str::to_owned),
                ..NodeReport::default()
            }
        })
        .collect()
}

/// A report and how far the phases that filled it got.
struct Attempt {
    report: ReviewReport,
    outcome: PyResult<()>,
}

/// The findings of failed audits and unit tests.
fn check_findings(build: &BuildOutcome, tests: &ValidationCounts) -> Vec<ReviewFinding> {
    let audits = build
        .failed_audits
        .iter()
        .map(|(node, audit)| ReviewFinding {
            node: node.clone(),
            kind: AUDIT_FINDING.to_owned(),
            metric: format!("audit {audit} failed"),
            limit: PASSES_LIMIT.to_owned(),
            ..ReviewFinding::default()
        });
    let unit_tests = tests.failures.iter().map(|failure| ReviewFinding {
        node: failure.trim_start_matches(UNIT_TEST_LABEL).to_owned(),
        kind: TEST_FINDING.to_owned(),
        metric: "unit test failed".to_owned(),
        limit: PASSES_LIMIT.to_owned(),
        ..ReviewFinding::default()
    });
    audits.chain(unit_tests).collect()
}

/// A built node's status from its build result, audits and comparison.
fn settled_status(
    node: &NodeReport,
    build: &BuildOutcome,
    with_findings: &BTreeSet<String>,
) -> (String, Option<String>) {
    let status: Option<&str> = node.build_status.as_deref();
    let blocked: bool = build.audit_blocked.contains(&node.name);
    let compared: bool = node
        .comparison
        .as_ref()
        .is_some_and(|comparison| comparison.complete && comparison.mode != NO_COMPARISON_MODE);
    let reason: Option<String> = node.reason.clone();
    match status {
        _ if with_findings.contains(&node.name) => (FINDING_NODE.to_owned(), reason),
        Some(SKIPPED_STATUS) => (
            NOT_VALIDATED_NODE.to_owned(),
            Some(SKIPPED_REASON.to_owned()),
        ),
        Some(SUCCESS_STATUS) | None if blocked => (FINDING_NODE.to_owned(), reason),
        Some(SUCCESS_STATUS) if compared => (OK_NODE.to_owned(), reason),
        Some(SUCCESS_STATUS) => (BUILT_NODE.to_owned(), reason),
        _ if blocked => (FINDING_NODE.to_owned(), reason),
        _ => (FAILED_NODE.to_owned(), reason),
    }
}

/// Checks that never ran: skipped builds and built seeds, which no comparison covers.
fn uncovered_checks(nodes: &[NodeReport], build: &BuildOutcome) -> Vec<IncompleteCheck> {
    nodes
        .iter()
        .filter_map(|node| {
            let status: Option<&str> = build.statuses.get(&node.name).map(String::as_str);
            let (check, reason) = match status {
                Some(SKIPPED_STATUS) => (BUILD_CHECK, SKIPPED_REASON),
                Some(SUCCESS_STATUS) if node.kind == SEED_KIND => {
                    (COMPARISON_CHECK, UNCOMPARED_SEED_REASON)
                }
                _ => return None,
            };
            Some(IncompleteCheck {
                node: node.name.clone(),
                check: check.to_owned(),
                reason: reason.to_owned(),
            })
        })
        .collect()
}

/// Settle each node's status from its build, audits, tests and comparison.
fn settle_nodes(
    report: ReviewReport,
    (build, comparisons): (&BuildOutcome, Comparisons),
    (affected, new): (&BTreeMap<String, Vec<String>>, &BTreeSet<String>),
) -> ReviewReport {
    let Comparisons {
        mut findings,
        mut by_node,
        mut incomplete,
    } = comparisons;
    findings.extend(check_findings(build, &report.tests));
    let with_findings: BTreeSet<String> = findings
        .iter()
        .map(|finding| finding.node.clone())
        .collect();
    let built: Vec<String> = report
        .nodes
        .iter()
        .filter(|node| node.status != NOT_VALIDATED_NODE)
        .map(|node| node.name.clone())
        .collect();
    let nodes: Vec<NodeReport> = report
        .nodes
        .into_iter()
        .map(|node| {
            let affected_columns: Vec<String> =
                affected.get(&node.name).cloned().unwrap_or_default();
            if !built.contains(&node.name) {
                return NodeReport {
                    affected_columns,
                    ..node
                };
            }
            let comparison = by_node.remove(&node.name);
            let new_relation: bool = new.contains(&node.name)
                && comparison
                    .as_ref()
                    .is_some_and(|comparison| comparison.mode == NO_COMPARISON_MODE);
            let built = NodeReport {
                affected_columns,
                build_status: build.statuses.get(&node.name).cloned(),
                comparison,
                reason: new_relation
                    .then(|| NEW_NODE_REASON.to_owned())
                    .or(node.reason.clone()),
                ..node
            };
            let (status, reason) = settled_status(&built, build, &with_findings);
            NodeReport {
                status,
                reason,
                ..built
            }
        })
        .collect();
    let built_nodes: Vec<NodeReport> = nodes
        .iter()
        .filter(|node| built.contains(&node.name))
        .cloned()
        .collect();
    incomplete.extend(uncovered_checks(&built_nodes, build));
    ReviewReport {
        nodes,
        findings,
        incomplete,
        ..report
    }
}

/// The directory the review's sub-runs write their result documents to.
fn work_directory(project: &ReviewProject, schema: &ReviewSchema) -> PyResult<PathBuf> {
    let work_dir: PathBuf = REVIEW_WORK_DIRECTORY
        .iter()
        .fold(project.project_dir.clone(), |path, part| path.join(part))
        .join(&schema.name);
    std::fs::create_dir_all(&work_dir)
        .map_err(|error| pyo3::exceptions::PyOSError::new_err(error.to_string()))?;
    Ok(work_dir)
}

/// A run's settings, plan and compiled project, then its connection config and connection.
type RunContext<'a, 'py> = (
    (&'a ReviewSettings, &'a ReviewProject, &'a Compiled<'py>),
    (&'a Bound<'py, PyAny>, &'a Bound<'py, PyAny>),
);

/// The models a comparison covers: validated models that built.
fn comparable<'a>(
    project: &'a ReviewProject,
    validated: &[String],
    build: &BuildOutcome,
) -> Vec<&'a ReviewNode> {
    validated
        .iter()
        .filter(|name| build.statuses.get(*name).map(String::as_str) == Some(SUCCESS_STATUS))
        .filter_map(|name| project.review_graph.nodes.get(name))
        .filter(|node| node.kind == MODEL_KIND)
        .collect()
}

/// Validated nodes new since the base revision.
fn new_nodes(report: &ReviewReport) -> BTreeSet<String> {
    report
        .plan
        .changed
        .iter()
        .filter(|node| node.added)
        .map(|node| node.name.clone())
        .collect()
}

/// Everything that needs the review schema: clone, build, tests, comparison and impact.
fn validate(
    (request, stream, machine): (&Bound<'_, PyAny>, &Bound<'_, PyAny>, bool),
    (((settings, project, compiled), (connection_config, connection)), schema): (
        RunContext<'_, '_>,
        &ReviewSchema,
    ),
    (review_discovered, sides): (&Bound<'_, PyAny>, Option<&Sides<'_>>),
    report: ReviewReport,
) -> Attempt {
    let mut report: ReviewReport = report;
    let outcome: PyResult<()> = (|| {
        let (_, adapter, graph) = compiled;
        let work_dir: PathBuf = work_directory(project, schema)?;
        let inputs: RunInputs<'_, '_> = RunInputs {
            request,
            project_dir: &project.project_dir,
            review_discovered,
            review_target: &schema.name,
            work_dir: &work_dir,
            output: (stream, machine),
        };
        let validated: Vec<String> = report.plan.validated.clone();
        let stateful: Vec<String> = validated
            .iter()
            .filter(|name| project.stateful.contains(*name))
            .cloned()
            .collect();
        report.incremental_starts = incremental_starts(&inputs, (settings, adapter), &stateful)?;
        let build: BuildOutcome = phase(
            stream,
            (
                &format!(
                    "Building {} changed and downstream nodes into {} (unchanged upstream from {})",
                    validated.len(),
                    schema.name,
                    settings.against
                ),
                "Built the review nodes",
                "Review build failed",
            ),
            || build_validated(&inputs, settings, (&project.review_graph, &validated)),
        )?;
        report.audits = build.audits.clone();
        report.tests = phase(
            stream,
            (
                &format!("Running unit tests affected since {}", project.reference),
                "Ran affected unit tests",
                "Unit tests failed to run",
            ),
            || run_changed_tests(&inputs, settings),
        )?;
        let models: Vec<&ReviewNode> = comparable(project, &validated, &build);
        let new: BTreeSet<String> = new_nodes(&report);
        let context: ComparisonContext<'_, '_> = ComparisonContext {
            request,
            discovered: review_discovered,
            adapter,
            connection,
            connection_config,
            review_target: &schema.name,
        };
        let comparisons: Comparisons = phase(
            stream,
            (
                &format!(
                    "Comparing {} models with {}",
                    models.len(),
                    settings.against
                ),
                "Compared the review models",
                "Comparison failed",
            ),
            || compare_models(&context, sides, (&models, &new)),
        )?;
        let affected_set: BTreeSet<String> =
            report.nodes.iter().map(|node| node.name.clone()).collect();
        let changed: Vec<String> = report
            .plan
            .changed
            .iter()
            .map(|node| node.name.clone())
            .collect();
        let affected: BTreeMap<String, Vec<String>> = affected_columns(
            &native_field(graph, "project")?,
            (&changed, &affected_set),
            &comparisons.by_node,
        )?;
        report = settle_nodes(
            std::mem::take(&mut report),
            (&build, comparisons),
            (&affected, &new),
        );
        Ok(())
    })();
    Attempt { report, outcome }
}

/// What dropping the review schema left: dropped, or the reason it was not.
fn dropped_state(py: Python<'_>, dropped: PyResult<bool>) -> PyResult<(bool, Option<String>)> {
    match dropped {
        Ok(true) => Ok((true, None)),
        Ok(false) => Ok((
            false,
            Some("the schema still exists after the drop".to_owned()),
        )),
        Err(error) => Ok((false, Some(error_parts(py, &error)?.message))),
    }
}

/// Name this run's review schema, refusing nodes that would build outside it, then connect.
fn open_review<'py>(
    request: &Bound<'py, PyAny>,
    (settings, project, compiled): (&ReviewSettings, &ReviewProject, &Compiled<'py>),
    report: &ReviewReport,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, PyAny>, ReviewSchema)> {
    let py: Python<'py> = request.py();
    let (discovered, adapter, _) = compiled;
    let run_id: String = report
        .invocation_id
        .clone()
        .unwrap_or_else(|| format!("{:x}", std::process::id()));
    let schema: ReviewSchema = review_schema(
        py,
        (project, settings, &report.plan.validated),
        (utc_now_compact(py)?, &report.baseline.head, &run_id),
    )?;
    let config: Bound<'py, PyAny> = connection_config(
        request,
        (&(&project.project_dir).into_pyobject(py)?, discovered),
        settings,
    )?;
    let connection: Bound<'py, PyAny> = adapter.call_method1("connect", (&config,))?;
    Ok((config, connection, schema))
}

/// The review target, and both sides of the comparison compiled before anything is created.
fn prepare<'py>(
    request: &Bound<'py, PyAny>,
    ((settings, project, compiled), (config, connection)): RunContext<'_, 'py>,
    (schema, report): (&ReviewSchema, &ReviewReport),
) -> PyResult<(Bound<'py, PyAny>, Option<Sides<'py>>)> {
    let (discovered, adapter, _) = compiled;
    let review_discovered: Bound<'py, PyAny> = with_review_target(discovered, settings, schema)?;
    let models: Vec<String> = report
        .plan
        .validated
        .iter()
        .filter(|name| {
            project
                .review_graph
                .nodes
                .get(*name)
                .is_some_and(|node| node.kind == MODEL_KIND)
        })
        .cloned()
        .collect();
    let context: ComparisonContext<'_, 'py> = ComparisonContext {
        request,
        discovered: &review_discovered,
        adapter,
        connection,
        connection_config: config,
        review_target: &schema.name,
    };
    let sides: Option<Sides<'py>> =
        prepare_sides(&context, settings, (&models, &new_nodes(report)))?;
    Ok((review_discovered, sides))
}

/// Create the review schema, validate inside it, and always drop it.
fn run_in_schema(
    (request, stream, machine): (&Bound<'_, PyAny>, &Bound<'_, PyAny>, bool),
    (settings, project, compiled): (&ReviewSettings, &ReviewProject, &Compiled<'_>),
    report: ReviewReport,
) -> Attempt {
    let py: Python<'_> = request.py();
    let (config, connection, schema) =
        match open_review(request, (settings, project, compiled), &report) {
            Ok(opened) => opened,
            Err(error) => {
                return Attempt {
                    report,
                    outcome: Err(error),
                };
            }
        };
    let adapter: &Bound<'_, PyAny> = &compiled.1;
    let run: RunContext<'_, '_> = ((settings, project, compiled), (&config, &connection));
    let mut state: ReviewSchemaReport = ReviewSchemaReport {
        database: schema.database.clone(),
        name: schema.name.clone(),
        ..ReviewSchemaReport::default()
    };
    let prepared = phase(
        stream,
        (
            &format!("Checking the relations to compare in {}", settings.against),
            "Checked the relations to compare",
            "Could not prepare the comparison",
        ),
        || prepare(request, run, (&schema, &report)),
    );
    let created: PyResult<(Bound<'_, PyAny>, Option<Sides<'_>>)> = prepared.and_then(|ready| {
        phase(
            stream,
            (
                &format!("Creating review schema {}", schema.name),
                "Created the review schema",
                "Could not create the review schema",
            ),
            || create_review_schema((adapter, &connection), &schema),
        )
        .map(|()| ready)
    });
    let attempt: Attempt = match created {
        Ok((review_discovered, sides)) => {
            state.created = true;
            validate(
                (request, stream, machine),
                (run, &schema),
                (&review_discovered, sides.as_ref()),
                report,
            )
        }
        Err(error) => Attempt {
            report,
            outcome: Err(error),
        },
    };
    let mut cleanup: PyResult<()> = Ok(());
    if state.created {
        let dropped: PyResult<bool> = phase(
            stream,
            (
                &format!("Dropping review schema {}", schema.name),
                "Dropped the review schema",
                "Could not drop the review schema",
            ),
            || drop_review_schema((adapter, &connection), &schema),
        );
        match dropped_state(py, dropped) {
            Ok((dropped, error)) => (state.dropped, state.error) = (dropped, error),
            Err(error) => cleanup = Err(error),
        }
    }
    let _ = adapter.call_method1("close", (&connection,));
    Attempt {
        report: ReviewReport {
            schema: Some(state),
            ..attempt.report
        },
        outcome: attempt.outcome.and(cleanup),
    }
}

/// Settings, planning and, unless this is a dry run, the run in the review schema.
fn review(
    (request, stream, machine): (&Bound<'_, PyAny>, &Bound<'_, PyAny>, bool),
    report: ReviewReport,
) -> Attempt {
    let mut report: ReviewReport = report;
    let planned: PyResult<Option<(ReviewSettings, ReviewProject, Compiled<'_>)>> = (|| {
        let py: Python<'_> = request.py();
        let (project_dir, discovered) = discover(request)?;
        let settings: ReviewSettings = review_settings(request, (&project_dir, &discovered))?;
        report.baseline.against.clone_from(&settings.against);
        report.baseline.target = Some(settings.target.clone());
        report.baseline.since_ref.clone_from(&settings.since);
        report.plan.policy = settings.policy;
        report.baseline.head = head_commit(&project_dir.extract::<PathBuf>()?).unwrap_or_default();
        let (project, compiled): (ReviewProject, Compiled<'_>) = phase(
            stream,
            (
                "Planning the review",
                "Planned the review",
                "Planning failed",
            ),
            || review_project(request, (project_dir, discovered), &settings, stream),
        )?;
        report.baseline.since_ref.clone_from(&project.reference);
        report.baseline.merge_base.clone_from(&project.merge_base);
        report.changes_json = project.changes_json.clone();
        report.changed_declarations = project.changed_declarations.clone();
        let tokens = selected_tokens(request, &compiled.2)?;
        report.plan = plan_review(
            &project.review_graph,
            &project.changed,
            settings.policy,
            tokens.as_deref(),
        )
        .map_err(|error| plan_error(py, &error))?;
        report.impact_edges = impact_edges(&project, &report.plan);
        report.nodes = planned_nodes(&project, &report.plan);
        report.deferred = deferred(&project, &report.plan.validated);
        write_line(
            stream,
            &format!(
                "Review plan: {} changed, {} to build, {} left out (--downstream {})",
                report.plan.changed.len(),
                report.plan.validated.len(),
                report.plan.not_validated.len(),
                downstream_policy_name(settings.policy),
            ),
        )?;
        if !report.plan.hooks.is_empty() {
            write_line(
                stream,
                &format!(
                    "Hooks run against the review schema: {}",
                    report.plan.hooks.join(", ")
                ),
            )?;
        }
        let skip: bool = report.dry_run || report.plan.validated.is_empty();
        Ok((!skip).then_some((settings, project, compiled)))
    })();
    match planned {
        Ok(Some((settings, project, compiled))) => run_in_schema(
            (request, stream, machine),
            (&settings, &project, &compiled),
            report,
        ),
        Ok(None) => Attempt {
            report,
            outcome: Ok(()),
        },
        Err(error) => Attempt {
            report,
            outcome: Err(error),
        },
    }
}

/// Write the Markdown report to `--markdown`, when given.
fn write_markdown(request: &Bound<'_, PyAny>, report: &ReviewReport) -> PyResult<Option<String>> {
    let Some(path) = native_field(request, "markdown_path")?.extract::<Option<PathBuf>>()? else {
        return Ok(None);
    };
    let written: std::io::Result<()> = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map_or(Ok(()), std::fs::create_dir_all)
        .and_then(|()| std::fs::write(&path, render_review_markdown(report)));
    Ok(written.err().map(|error| {
        format!(
            "could not write the Markdown report to {}: {error}",
            path.display()
        )
    }))
}

/// Print the report to stdout, as JSON or text.
fn print_report(py: Python<'_>, report: &ReviewReport, machine: bool) -> PyResult<()> {
    let document: String = if machine {
        render_review_json(report)
    } else {
        render_review_text(report)
    };
    write_stdout_bytes(py, document.as_bytes())
}

/// `report` failed by the Markdown write failure, unless an earlier error already failed it.
fn with_markdown_failure(report: ReviewReport, failure: Option<&String>) -> ReviewReport {
    match (&report.error, failure) {
        (None, Some(message)) => ReviewReport {
            error: Some(ReviewFailure {
                code: MARKDOWN_WRITE_CODE.to_owned(),
                message: message.clone(),
                help: Some("pass a --markdown path in a writable directory".to_owned()),
            }),
            ..report
        },
        _ => report,
    }
}

/// Run one `sqb review` request.
pub(crate) fn run_review_command(py: Python<'_>, request: &Bound<'_, PyAny>) -> PyResult<i32> {
    let started: Instant = Instant::now();
    let machine: bool = native_field(request, "json_output")?.is_truthy()?;
    let debug: bool = native_field(request, "debug")?.is_truthy()?;
    let stream: Bound<'_, PyAny> = progress_stream(py, machine || debug)?;
    let started_report: ReviewReport = ReviewReport {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        dry_run: native_field(request, "dry_run")?.is_truthy()?,
        invocation_id: invocation_id(py)?,
        ..ReviewReport::default()
    };
    let Attempt { report, outcome } = review((request, &stream, machine), started_report);
    let cause: Option<PyErr> = outcome.err();
    let report: ReviewReport = ReviewReport {
        duration_seconds: started.elapsed().as_secs_f64(),
        error: cause
            .as_ref()
            .map(|error| error_parts(py, error))
            .transpose()?,
        ..report
    };
    let markdown_failure: Option<String> = write_markdown(request, &report)?;
    let report: ReviewReport = with_markdown_failure(report, markdown_failure.as_ref());
    print_report(py, &report, machine)?;
    if let Some(cause) = cause {
        if !cause.is_instance_of::<PyException>(py) {
            return Err(cause);
        }
        if let Some(message) = &markdown_failure {
            write_stderr(py, &format!("{message}\n"))?;
        }
        let error = report.error.clone().unwrap_or_default();
        return Err(failed_error(py, &error, cause));
    }
    if let Some(error) = &report.error {
        return Err(failed_error(
            py,
            error,
            pyo3::exceptions::PyOSError::new_err(error.message.clone()),
        ));
    }
    Ok(review_outcome(&report).1)
}
