//! Run built-in, SQL and custom rule phases on their threads and combine their findings.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::thread::JoinHandle;
use std::time::Duration;

use pyo3::exceptions::{PyKeyboardInterrupt, PyRuntimeError};
use pyo3::prelude::{
    Bound, Py, PyAny, PyAnyMethods, PyErr, PyModule, PyModuleMethods, PyResult, Python,
};
use pyo3::types::{PyDict, PyTuple, PyTupleMethods};
use pyo3::{IntoPyObjectExt, pyfunction, wrap_pyfunction};

use sqlbuild_host::entries::models::UserCodeHost;

use crate::bindings::_helpers::boundary::python_calls::{KeywordArguments, call_with, keywords};
use crate::bindings::_helpers::cache::native_store::current_code_identity;
use crate::bindings::_helpers::compile_driver::stage_work::count_stage;
use crate::bindings::_helpers::compiled_project::project::NativeCompiledProject;
use crate::bindings::_helpers::native_host::host;
use crate::bindings::_helpers::records::construct::new_record_with;
use crate::bindings::_helpers::records::fields::field;
use crate::bindings::_helpers::records::fields::replaced;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::rules::catalogue::{
    resolved_ignore_selectors, rule_catalogue, select_rules,
};
use crate::bindings::_helpers::rules::constants::RULES_MEMO_REUSES;
use crate::bindings::_helpers::rules::constants::TYPE_PROOF_RULE_CODES;
use crate::bindings::_helpers::rules::custom_evaluation::{
    CustomRun, evaluate_custom_rules_cached, outcome,
};
use crate::bindings::_helpers::rules::custom_hosts::HostCancel;
use crate::bindings::_helpers::rules::custom_rule_evidence::verify_custom_rules;
use crate::bindings::_helpers::rules::early_builtin_rules::{
    EarlyBuiltinRules, EarlyRulesReport, take_early_rules,
};
use crate::bindings::_helpers::rules::fact_replay::project_models;
use crate::bindings::_helpers::rules::results::rule_finding;
use crate::bindings::_helpers::rules::rows::finalize_rule_findings_rows;
use crate::bindings::_helpers::rules::rows::{
    NativeRulesRequest, build_rules_request, evaluate_rules_request, evaluate_rules_request_with,
    rules_request_over, selectionless_rules_request,
};
use crate::bindings::_helpers::rules::rule_plan::{
    plan_rule_run, selects_row_rules, whole_project_codes,
};
use crate::bindings::_helpers::rules::rule_run::finish_rule_run;
use crate::bindings::_helpers::rules::selection::{rule_selected, selected_rule_codes};
use crate::bindings::_helpers::rules::sql_rules::run_planned_sql_rules;
use sqlbuild_rules::models::RowsRequest;
use std::sync::Arc;

const CUSTOM_THREAD: &str = "sqlbuild-custom-rules";
const GENERIC_DIALECT: &str = "generic";
const PROJECT_RULES_OPTIONS: (&[&str], &[&str]) = (
    &["project", "config", "project_dir"],
    &["dialect", "verify_determinism"],
);
const NATIVE_RULES_OPTIONS: (&[&str], &[&str]) = (
    &["project", "config", "project_dir", "catalogue"],
    &[
        "dialect",
        "initial_findings",
        "defer_suppressions",
        "verify_determinism",
    ],
);
const CUSTOM_RULES_OPTIONS: (&[&str], &[&str]) = (
    &["project", "config", "project_dir", "rules", "dialect"],
    &["verify_determinism", "implementation_fingerprints"],
);
const RULES_THREAD: &str = "sqlbuild-rules";
/// How often a thread waiting for a rule phase checks for interrupts.
const JOIN_POLL: Duration = Duration::from_millis(100);
const EVALUATE_RULES_OPTIONS: (&[&str], &[&str]) = (
    &[
        "graph",
        "discovered_inputs",
        "config",
        "project_dir",
        "dialect",
    ],
    &["selected_keys", "early_lint", "no_sql_analysis"],
);

/// A running custom-rule evaluation, whose result is a `CustomRulesOutcome`.
type CustomHandle = PhaseThread;

/// A rule phase thread and the channel that disconnects when it finishes.
struct PhaseThread {
    handle: JoinHandle<PyResult<Py<PyAny>>>,
    finished: Receiver<()>,
}

impl PhaseThread {
    fn spawn(
        name: &str,
        phase: impl FnOnce() -> PyResult<Py<PyAny>> + Send + 'static,
    ) -> PyResult<Self> {
        let (sender, finished): (Sender<()>, Receiver<()>) = channel();
        let handle: JoinHandle<PyResult<Py<PyAny>>> = std::thread::Builder::new()
            .name(name.to_owned())
            .spawn(move || {
                let result: PyResult<Py<PyAny>> = phase();
                drop(sender);
                result
            })
            .map_err(|error| PyRuntimeError::new_err(error.to_string()))?;
        Ok(Self { handle, finished })
    }
}

/// The rule run one native evaluation describes; every field stays bound to its Python value.
struct NativeRun<'py> {
    project: Bound<'py, PyAny>,
    config: Bound<'py, PyAny>,
    project_dir: Bound<'py, PyAny>,
    catalogue: Bound<'py, PyAny>,
    dialect: String,
    verify_determinism: bool,
    initial_findings: Option<Bound<'py, PyAny>>,
    cancel: HostCancel,
}

/// How a native evaluation obtains its custom-rule outcome.
enum CustomSource {
    /// Already started (or nothing to start) by the caller.
    Started(Option<CustomHandle>),
    /// Start once the native request is built, beside the native evaluation.
    Deferred,
}

/// Evaluate the selected compiler rules over compiled models.
#[pyfunction]
#[pyo3(signature = (**options))]
fn evaluate_project_rules<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "evaluate_project_rules", options, PROJECT_RULES_OPTIONS)?;
    let project: Bound<'py, PyAny> = arguments.value("project")?;
    let config: Bound<'py, PyAny> = arguments.value("config")?;
    let project_dir: Bound<'py, PyAny> = arguments.value("project_dir")?;
    let dialect: String = dialect_argument(&arguments)?;
    let verify_determinism: bool = arguments.flag("verify_determinism")?;
    let effective: Bound<'py, PyAny> = resolved_ignore_selectors(&config, &project)?;
    let rules: Vec<Bound<'py, PyAny>> = rule_catalogue(&effective, &project_dir, true)?;
    let mut custom: bool = false;
    for rule in &rules {
        custom = custom || field(rule, "custom")?.is_truthy()?;
    }
    if custom {
        let selected: Vec<Bound<'py, PyAny>> =
            select_rules(&rules, &effective, &project_dir, None)?;
        verify_custom_rules(PyTuple::new(py, selected)?.as_any(), &project_dir)?;
    }
    let catalogue: Bound<'py, PyAny> = PyTuple::new(py, rules)?.into_any();
    let run: NativeRun<'py> = NativeRun {
        project,
        config: effective,
        project_dir,
        catalogue,
        dialect,
        verify_determinism,
        initial_findings: None,
        cancel: HostCancel::default(),
    };
    evaluate_native(&run, (false, None), CustomSource::Deferred, None)
}

/// Evaluate built-in rules natively with the catalogue's selected custom rules beside them.
#[pyfunction]
#[pyo3(signature = (**options))]
fn evaluate_native_rules<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "evaluate_native_rules", options, NATIVE_RULES_OPTIONS)?;
    let initial_findings: Bound<'py, PyAny> = arguments.value("initial_findings")?;
    let defer_suppressions: bool = arguments.flag("defer_suppressions")?;
    let run: NativeRun<'py> = NativeRun {
        project: arguments.value("project")?,
        config: arguments.value("config")?,
        project_dir: arguments.value("project_dir")?,
        catalogue: arguments.value("catalogue")?,
        dialect: dialect_argument(&arguments)?,
        verify_determinism: arguments.flag("verify_determinism")?,
        initial_findings: (!initial_findings.is_none()).then_some(initial_findings),
        cancel: HostCancel::default(),
    };
    evaluate_native(
        &run,
        (defer_suppressions, None),
        CustomSource::Deferred,
        None,
    )
}

/// Evaluate custom rules through their incremental cache.
#[pyfunction]
#[pyo3(signature = (**options))]
fn evaluate_custom_rules<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "evaluate_custom_rules", options, CUSTOM_RULES_OPTIONS)?;
    let fingerprints: Bound<'py, PyAny> = arguments.value("implementation_fingerprints")?;
    evaluate_custom_rules_cached(CustomRun {
        project: &arguments.value("project")?,
        config: &arguments.value("config")?,
        project_dir: &arguments.value("project_dir")?,
        rules: arguments
            .value("rules")?
            .try_iter()?
            .collect::<PyResult<_>>()?,
        dialect: &arguments.value("dialect")?.extract::<String>()?,
        verify_determinism: arguments.flag("verify_determinism")?,
        implementation_fingerprints: if fingerprints.is_none() {
            HashMap::new()
        } else {
            fingerprints.extract()?
        },
        cancel: HostCancel::default(),
        cache_directory: None,
    })
}

/// The `dialect` argument, `"generic"` when it was not passed.
fn dialect_argument(arguments: &KeywordArguments<'_>) -> PyResult<String> {
    if arguments.passed("dialect")? {
        return arguments.value("dialect")?.extract();
    }
    Ok(GENERIC_DIALECT.to_owned())
}

/// Evaluate independent rule phases concurrently, then finalize their combined findings.
#[pyfunction]
#[pyo3(signature = (**options))]
pub(crate) fn evaluate_rules<'py>(
    py: Python<'py>,
    options: Option<Bound<'py, PyDict>>,
) -> PyResult<Bound<'py, PyAny>> {
    let arguments: KeywordArguments<'py> =
        KeywordArguments::read(py, "evaluate_rules", options, EVALUATE_RULES_OPTIONS)?;
    let graph: Bound<'py, PyAny> = arguments.value("graph")?;
    let dialect: Bound<'py, PyAny> = arguments.value("dialect")?;
    let graph_project: Bound<'py, PyAny> = field(&graph, "project")?;
    let plan: Bound<'py, PyAny> = plan_rule_run(Some(&keywords(
        py,
        &[
            ("graph", graph.clone()),
            ("config", arguments.value("config")?),
            ("project_dir", arguments.value("project_dir")?),
            ("selected_keys", arguments.value("selected_keys")?),
            (
                "no_sql_analysis",
                arguments.value_or_false("no_sql_analysis")?,
            ),
        ],
    )?))?;
    let whole_codes: Vec<String> = whole_project_codes(&plan)?;
    let selection: bool = !field(&plan, "model_paths")?.is_none();
    let run: NativeRun<'py> = NativeRun {
        project: if selection {
            graph_project.clone()
        } else {
            field(&plan, "project")?
        },
        config: if selection {
            whole_project_config(&plan)?
        } else {
            field(&plan, "config")?
        },
        project_dir: field(&plan, "project_dir")?,
        catalogue: field(&plan, "catalogue")?,
        dialect: dialect.extract()?,
        verify_determinism: false,
        initial_findings: None,
        cancel: HostCancel::default(),
    };
    let payloads: Bound<'py, PyAny> = field(&plan, "custom_payloads")?;
    let custom: Option<CustomHandle> = start_custom_rules(&run, &payloads)?;
    let native_run: [Py<PyAny>; 5] = [
        run.project.clone().unbind(),
        run.config.clone().unbind(),
        run.project_dir.clone().unbind(),
        run.catalogue.clone().unbind(),
        payloads.clone().unbind(),
    ];
    let native_dialect: String = run.dialect.clone();
    let native_cancel: HostCancel = run.cancel.clone();
    let selectionless: bool = custom.is_none() && !selects_row_rules(&plan)?;
    let early: Option<EarlyBuiltinRules> = take_early_rules();
    let early_report: Option<Arc<EarlyRulesReport>> = early.as_ref().map(EarlyBuiltinRules::report);
    let native_thread: Option<PhaseThread> = if selectionless {
        if let Some(early) = early {
            early.discard(py);
        }
        None
    } else {
        Some(PhaseThread::spawn(RULES_THREAD, move || {
            Python::attach(|py| {
                let [project, config, project_dir, catalogue, payloads] = native_run;
                let run: NativeRun<'_> = NativeRun {
                    project: project.into_bound(py),
                    config: config.into_bound(py),
                    project_dir: project_dir.into_bound(py),
                    catalogue: catalogue.into_bound(py),
                    dialect: native_dialect,
                    verify_determinism: false,
                    initial_findings: None,
                    cancel: native_cancel,
                };
                evaluate_native(
                    &run,
                    (true, Some(payloads.into_bound(py))),
                    CustomSource::Started(custom),
                    early,
                )
                .map(Bound::unbind)
            })
        })?)
    };
    let sql_result: PyResult<Bound<'py, PyAny>> = run_planned_sql_rules(Some(&keywords(
        py,
        &[
            ("plan", plan.clone()),
            ("graph", graph),
            ("discovered_inputs", arguments.value("discovered_inputs")?),
            ("dialect", dialect),
            ("early_lint", arguments.value("early_lint")?),
        ],
    )?));
    cancel_on_interrupt(py, &sql_result, &run.cancel);
    let result: PyResult<Bound<'py, PyAny>> = match native_thread {
        Some(native_thread) => joined(py, native_thread, &run.cancel),
        None => selectionless_result(&run, &payloads),
    };
    if let Some(report) = early_report {
        report.record();
    }
    let sql_result: Bound<'py, PyAny> = sql_result?;
    let result: Bound<'py, PyAny> = result?;
    let result: Bound<'py, PyAny> = if selection {
        selected_findings(&plan, &whole_codes, &run, &result)?
    } else {
        result
    };
    finish_rule_run(&plan, &sql_result, &result)
}

/// The rules config a whole-project compile evaluates: every selected rule and every exception.
fn whole_project_config<'py>(plan: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = plan.py();
    let mut codes: Vec<Bound<'py, PyAny>> = Vec::new();
    for rule in field(plan, "selected")?.try_iter()? {
        codes.push(field(&rule?, "code")?);
    }
    replaced(
        &field(plan, "effective_config")?,
        &[
            ("select", PyTuple::new(py, codes)?.into_any()),
            ("ignore", PyTuple::empty(py).into_any()),
        ],
    )
}

/// `whole`'s `codes` findings kept to the selected models and to files that are not models.
fn selected_findings<'py>(
    plan: &Bound<'py, PyAny>,
    codes: &[String],
    run: &NativeRun<'py>,
    whole: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = plan.py();
    let finalized: Bound<'py, PyTuple> = finalize_rule_findings_rows(
        &run.config,
        &run.project_dir,
        codes.to_vec(),
        field(whole, "findings")?
            .try_iter()?
            .collect::<PyResult<_>>()?,
    )?;
    let selected: Bound<'py, PyAny> = field(plan, "model_paths")?;
    let models: HashSet<String> = project_models(&run.project)?
        .into_iter()
        .map(|model| model.posix)
        .collect();
    let mut findings: Vec<Bound<'py, PyAny>> = Vec::new();
    for finding in finalized.iter() {
        let path: String = rule_finding(&finding)?.path;
        if !models.contains(&path) || selected.contains(&path)? {
            findings.push(finding);
        }
    }
    replaced(
        whole,
        &[
            ("findings", PyTuple::new(py, findings)?.into_any()),
            (
                "evaluated_models",
                project_models(&field(plan, "project")?)?
                    .len()
                    .into_bound_py_any(py)?,
            ),
        ],
    )
}

/// Evaluate built-in rules natively, or from `early` when identical, beside the custom rules.
fn evaluate_native<'py>(
    run: &NativeRun<'py>,
    (defer_suppressions, custom_payloads): (bool, Option<Bound<'py, PyAny>>),
    custom: CustomSource,
    early: Option<EarlyBuiltinRules>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = run.project.py();
    let prepared: PyResult<(Bound<'py, PyAny>, Bound<'py, NativeRulesRequest>)> =
        native_request(run, custom_payloads);
    let custom: Option<CustomHandle> = match (custom, &prepared) {
        (CustomSource::Started(handle), Ok(_)) => handle,
        (CustomSource::Started(handle), Err(_)) => {
            if let Some(handle) = handle {
                let _abandoned: PyResult<Bound<'py, PyAny>> = joined(py, handle, &run.cancel);
            }
            None
        }
        (CustomSource::Deferred, Ok((payloads, _))) => start_custom_rules(run, payloads)?,
        (CustomSource::Deferred, Err(_)) => None,
    };
    let (_, request): (Bound<'py, PyAny>, Bound<'py, NativeRulesRequest>) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            if let Some(early) = early {
                early.discard(py);
            }
            return Err(error);
        }
    };
    let evaluated: PyResult<Bound<'py, PyAny>> = match early {
        Some(early) => evaluate_rules_request_with(py, &request, early, &run.cancel),
        None => evaluate_rules_request(py, &request),
    };
    cancel_on_interrupt(py, &evaluated, &run.cancel);
    let custom: PyResult<Bound<'py, PyAny>> = match custom {
        Some(handle) => joined(py, handle, &run.cancel),
        None => outcome(py, (Vec::new(), 0, 0), (0, 0), &[]),
    };
    rules_result(run, defer_suppressions, evaluated, custom)
}

/// The built-in rules result of a run that selects no rows-engine rules, without building its models.
fn selectionless_result<'py>(
    run: &NativeRun<'py>,
    payloads: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = run.project.py();
    let request: Bound<'py, NativeRulesRequest> = Bound::new(
        py,
        selectionless_rules_request(&run.config, &run.project_dir, &run.dialect, payloads)?,
    )?;
    let evaluated: PyResult<Bound<'py, PyAny>> = evaluate_rules_request(py, &request);
    let result: Bound<'py, PyAny> = rules_result(
        run,
        true,
        evaluated,
        outcome(py, (Vec::new(), 0, 0), (0, 0), &[]),
    )?;
    let evaluated_models: usize = if field(&run.config, "select")?.len()? == 0 {
        0
    } else {
        project_models(&run.project)?.len()
    };
    replaced(
        &result,
        &[("evaluated_models", evaluated_models.into_bound_py_any(py)?)],
    )
}

/// Combine one native evaluation with its custom-rule outcome into a `RulesResult`.
fn rules_result<'py>(
    run: &NativeRun<'py>,
    defer_suppressions: bool,
    evaluated: PyResult<Bound<'py, PyAny>>,
    custom: PyResult<Bound<'py, PyAny>>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = run.project.py();
    let native_result: Bound<'py, PyAny> = evaluated?;
    let custom: Bound<'py, PyAny> = custom?;
    let combined: Vec<Bound<'py, PyAny>> = field(&native_result, "findings")?
        .try_iter()?
        .chain(field(&custom, "findings")?.try_iter()?)
        .collect::<PyResult<Vec<_>>>()?;
    let findings: Bound<'py, PyAny> = if defer_suppressions {
        PyTuple::new(py, combined)?.into_any()
    } else {
        finalize_rule_findings_rows(
            &run.config,
            &run.project_dir,
            field(&native_result, "selected_codes")?.extract()?,
            combined,
        )?
        .into_any()
    };
    let reused: bool = field(&native_result, "reused")?.is_truthy()?;
    if reused {
        count_stage(py, RULES_MEMO_REUSES, 1)?;
    }
    let native_hits: i64 = field(&native_result, "cache_hits")?.extract()?;
    let native_misses: i64 = field(&native_result, "cache_misses")?.extract()?;
    let custom_hits: i64 = field(&custom, "cache_hits")?.extract()?;
    let custom_misses: i64 = field(&custom, "cache_misses")?.extract()?;
    new_record_with(
        py,
        RecordId::RulesResult,
        &[
            ("findings", findings),
            (
                "evaluated_models",
                field(&native_result, "evaluated_models")?,
            ),
            (
                "cache_hits",
                (if reused {
                    native_hits + native_misses
                } else {
                    native_hits
                } + custom_hits)
                    .into_bound_py_any(py)?,
            ),
            (
                "cache_misses",
                (if reused { 0 } else { native_misses } + custom_misses).into_bound_py_any(py)?,
            ),
            (
                "built_in_ms",
                if reused {
                    0.into_bound_py_any(py)?
                } else {
                    field(&native_result, "built_in_ms")?
                },
            ),
            ("custom_ms", field(&custom, "custom_ms")?),
            ("custom_cpu_ms", field(&custom, "custom_cpu_ms")?),
            (
                "custom_rule_timings",
                field(&custom, "custom_rule_timings")?,
            ),
        ],
    )
}

/// The custom-rule payloads and the native rules request of one evaluation.
fn native_request<'py>(
    run: &NativeRun<'py>,
    custom_payloads: Option<Bound<'py, PyAny>>,
) -> PyResult<(Bound<'py, PyAny>, Bound<'py, NativeRulesRequest>)> {
    let py: Python<'py> = run.project.py();
    let payloads: Bound<'py, PyAny> = match custom_payloads {
        Some(payloads) => payloads,
        None => call_with(
            &host::entry(py, UserCodeHost::CustomRulePayloads)?,
            &[
                ("catalogue", run.catalogue.clone()),
                ("project_dir", run.project_dir.clone()),
            ],
        )?,
    };
    let request: Bound<'py, NativeRulesRequest> = Bound::new(
        py,
        build_rules_request(Some(&request_options(run, &payloads)?))?,
    )?;
    Ok((payloads, request))
}

/// The `build_rules_request` options of one native evaluation with its custom-rule payloads.
fn request_options<'py>(
    run: &NativeRun<'py>,
    payloads: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    let py: Python<'py> = run.project.py();
    let mut include_type_proof: bool = false;
    for code in TYPE_PROOF_RULE_CODES {
        include_type_proof = include_type_proof || rule_selected(&run.config, code)?;
    }
    let options: Bound<'py, PyDict> = keywords(
        py,
        &[
            ("project", run.project.clone()),
            ("config", run.config.clone()),
            ("project_dir", run.project_dir.clone()),
            ("dialect", run.dialect.clone().into_bound_py_any(py)?),
            (
                "initial_findings",
                run.initial_findings
                    .clone()
                    .unwrap_or_else(|| PyTuple::empty(py).into_any()),
            ),
            ("custom_payloads", payloads.clone()),
            (
                "include_type_proof",
                include_type_proof.into_bound_py_any(py)?,
            ),
        ],
    )?;
    Ok(options)
}

/// The request `evaluate_rules` would build for `plan`, reading model facts from `facts`.
pub(crate) fn speculative_rules_request<'py>(
    plan: &Bound<'py, PyAny>,
    dialect: &str,
    facts: &Bound<'py, NativeCompiledProject>,
) -> PyResult<RowsRequest> {
    let run: NativeRun<'py> = NativeRun {
        project: field(plan, "project")?,
        config: field(plan, "config")?,
        project_dir: field(plan, "project_dir")?,
        catalogue: field(plan, "catalogue")?,
        dialect: dialect.to_owned(),
        verify_determinism: false,
        initial_findings: None,
        cancel: HostCancel::default(),
    };
    rules_request_over(
        &request_options(&run, &field(plan, "custom_payloads")?)?,
        facts,
    )
}

/// Start the selected custom rules on their thread after digesting installed code here.
fn start_custom_rules(
    run: &NativeRun<'_>,
    payloads: &Bound<'_, PyAny>,
) -> PyResult<Option<CustomHandle>> {
    let py: Python<'_> = run.project.py();
    if !payloads.is_truthy()? {
        return Ok(None);
    }
    let selected: HashSet<String> = selected_rule_codes(&run.config, payloads)?
        .into_iter()
        .collect();
    let mut rules: Vec<Py<PyAny>> = Vec::new();
    for rule in run.catalogue.try_iter()? {
        let rule: Bound<'_, PyAny> = rule?;
        let code: String = field(&rule, "code")?.extract()?;
        if field(&rule, "custom")?.is_truthy()? && selected.contains(&code) {
            rules.push(rule.unbind());
        }
    }
    if rules.is_empty() {
        return Ok(None);
    }
    if field(&field(&run.config, "cache")?, "enabled")?.is_truthy()? {
        let _identity: String = current_code_identity(py)?;
    }
    let mut fingerprints: HashMap<String, String> = HashMap::new();
    for payload in payloads.try_iter()? {
        let payload: Bound<'_, PyAny> = payload?;
        fingerprints.insert(
            payload.get_item("code")?.str()?.extract()?,
            payload
                .get_item("implementation_fingerprint")?
                .str()?
                .extract()?,
        );
    }
    let inputs: [Py<PyAny>; 3] = [
        run.project.clone().unbind(),
        run.config.clone().unbind(),
        run.project_dir.call_method0("resolve")?.unbind(),
    ];
    let dialect: String = run.dialect.clone();
    let verify_determinism: bool = run.verify_determinism;
    let cancel: HostCancel = run.cancel.clone();
    PhaseThread::spawn(CUSTOM_THREAD, move || {
        Python::attach(|py| {
            let [project, config, project_dir] = inputs;
            evaluate_custom_rules_cached(CustomRun {
                project: project.bind(py),
                config: config.bind(py),
                project_dir: project_dir.bind(py),
                rules: rules.into_iter().map(|rule| rule.into_bound(py)).collect(),
                dialect: &dialect,
                verify_determinism,
                implementation_fingerprints: fingerprints,
                cancel,
                cache_directory: None,
            })
            .map(Bound::unbind)
        })
    })
    .map(Some)
}

/// Wait for a phase thread off the interpreter; an interrupt cancels the run's hosts first.
fn joined<'py>(
    py: Python<'py>,
    phase: PhaseThread,
    cancel: &HostCancel,
) -> PyResult<Bound<'py, PyAny>> {
    let finished: Mutex<Receiver<()>> = Mutex::new(phase.finished);
    let mut signal: Option<PyErr> = None;
    loop {
        let waited: Result<(), RecvTimeoutError> = py.detach(|| match finished.lock() {
            Ok(receiver) => receiver.recv_timeout(JOIN_POLL),
            Err(_) => Err(RecvTimeoutError::Disconnected),
        });
        match waited {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {
                if signal.is_none()
                    && let Err(error) = py.check_signals()
                {
                    cancel.store(true, Ordering::Release);
                    signal = Some(error);
                }
            }
        }
    }
    let joined: PyResult<Py<PyAny>> = py
        .detach(|| phase.handle.join())
        .map_err(|_| PyRuntimeError::new_err("rules thread panicked"))?;
    if let Some(error) = signal {
        return Err(error);
    }
    joined.map(|value| value.into_bound(py))
}

/// Cancel the run's custom-rule hosts when `result` is an interrupt.
fn cancel_on_interrupt(py: Python<'_>, result: &PyResult<Bound<'_, PyAny>>, cancel: &HostCancel) {
    if let Err(error) = result
        && error.is_instance_of::<PyKeyboardInterrupt>(py)
    {
        cancel.store(true, Ordering::Release);
    }
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(evaluate_project_rules, module)?)?;
    module.add_function(wrap_pyfunction!(evaluate_native_rules, module)?)?;
    module.add_function(wrap_pyfunction!(evaluate_custom_rules, module)?)?;
    module.add_function(wrap_pyfunction!(evaluate_rules, module)?)
}
