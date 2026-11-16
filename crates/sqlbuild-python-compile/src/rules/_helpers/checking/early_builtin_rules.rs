//! Built-in rule evaluation started speculatively once a compile's model types are recovered.

use crate::rules::main::configuration::load_rules_config_with_cache::load_rules_config_with_cache;
use crate::rules::models::EarlyRulesSettings;
use std::cell::RefCell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use pyo3::prelude::{Bound, IntoPyObject, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_host::compile_context::main::add_compile_timing::add_compile_timing;
use sqlbuild_host::compile_context::main::record_compile_metric::record_compile_metric;
use sqlbuild_rules::engine::main::answer_speculative_rows::answer_speculative_rows;
use sqlbuild_rules::engine::main::discard_speculative_rows::discard_speculative_rows;
use sqlbuild_rules::engine::main::start_speculative_rows::start_speculative_rows;
use sqlbuild_rules::errors::RowsError;
use sqlbuild_rules::models::{RowsEvaluation, RowsRequest, SpeculativeAnswer, SpeculativeRows};

use crate::rules::_helpers::configuration::catalogue::references_custom_rules;
use crate::rules::_helpers::rule::rule_dispatch::speculative_rules_request;
use crate::rules::_helpers::rule::rule_plan::rule_run_plan;
use crate::rules::_helpers::rule::rule_plan::selects_row_rules;
use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_base::types::CompilerDetach;
use sqlbuild_python_project::compiled_project::models::NativeCompiledProject;
use sqlbuild_python_project::compiled_views::main::project::compiled_project_facts::compiled_project_facts;
use sqlbuild_python_project::compiled_views::main::project::record_model_analyses::record_model_analyses;

const CANCELLED: &str = "native rules evaluation was cancelled";
pub(crate) const BUILD_PHASE: &str = "early_rules_build_ms";
const WAIT_PHASE: &str = "early_rules_wait_ms";
const HIT_METRIC: &str = "early_rules_hits";
const MISS_METRIC: &str = "early_rules_misses";

/// One compile thread's early built-in rules, from arming to the rules phase taking them.
pub(crate) enum Slot {
    Armed(EarlyRulesSettings),
    Started(EarlyBuiltinRules),
    Spent,
}

thread_local! {
    pub(crate) static SLOT: RefCell<Option<Slot>> = const { RefCell::new(None) };
}

/// Whether the rules phase used the early evaluation, and how long it waited for it.
#[derive(Default)]
pub(crate) struct EarlyRulesReport {
    used: AtomicBool,
    consulted: AtomicBool,
    wait_ns: std::sync::atomic::AtomicU64,
}

impl EarlyRulesReport {
    /// Record the outcome in the compile timings of the calling thread.
    pub(crate) fn record(&self) {
        if !self.consulted.load(Ordering::Acquire) {
            return;
        }
        add_compile_timing(WAIT_PHASE, self.wait_ns.load(Ordering::Acquire));
        let used: bool = self.used.load(Ordering::Acquire);
        record_compile_metric(HIT_METRIC, i64::from(used));
        record_compile_metric(MISS_METRIC, i64::from(!used));
    }
}

/// The started early rules, which only the rules phase of the arming compile takes.
pub(crate) fn take_early_rules() -> Option<EarlyBuiltinRules> {
    take_started()
}

pub(crate) fn take_started() -> Option<EarlyBuiltinRules> {
    SLOT.with(|slot| {
        let mut slot = slot.borrow_mut();
        match slot.as_mut() {
            Some(current) => match std::mem::replace(current, Slot::Spent) {
                Slot::Started(rules) => Some(rules),
                other => {
                    *current = other;
                    None
                }
            },
            None => None,
        }
    })
}

/// The built-in rules request the rules phase is expected to build, when it evaluates no custom rules.
pub(crate) fn early_request(
    project: &Bound<'_, PyAny>,
    settings: &EarlyRulesSettings,
) -> PyResult<Option<RowsRequest>> {
    let py: Python<'_> = project.py();
    let config: Bound<'_, PyAny> =
        load_rules_config_with_cache(py, settings.project_dir.clone(), !settings.no_cache)?
            .into_bound(py);
    if references_custom_rules(&config)? {
        return Ok(None);
    }
    let project_dir: Bound<'_, PyAny> = settings.project_dir.clone().into_pyobject(py)?.into_any();
    let plan: Bound<'_, PyAny> = rule_run_plan(
        (project, &config, &project_dir),
        None,
        settings.no_sql_analysis,
    )?;
    if !selects_row_rules(&plan)? || field(&plan, "custom_payloads")?.is_truthy()? {
        return Ok(None);
    }
    let facts: Bound<'_, NativeCompiledProject> = Bound::new(
        py,
        NativeCompiledProject::with_facts(
            compiled_project_facts(project)?.borrow().facts().clone(),
        ),
    )?;
    let _ = record_model_analyses(&facts, &field(project, "models")?)?;
    speculative_rules_request(&plan, &settings.dialect, &facts).map(Some)
}

/// One speculative built-in rules evaluation and the report of how the rules phase used it.
pub(crate) struct EarlyBuiltinRules {
    speculative: SpeculativeRows,
    report: Arc<EarlyRulesReport>,
}

impl EarlyBuiltinRules {
    /// The started speculation, or `None` when its thread cannot start.
    pub(crate) fn spawn(rows: RowsRequest) -> Option<Self> {
        match start_speculative_rows(rows) {
            Ok(speculative) => Some(Self {
                speculative,
                report: Arc::new(EarlyRulesReport::default()),
            }),
            Err(_) => None,
        }
    }

    /// The outcome report the rules phase records once it has used or discarded these rules.
    pub(crate) fn report(&self) -> Arc<EarlyRulesReport> {
        Arc::clone(&self.report)
    }

    /// Evaluate `rows`, from this speculation only when identical; `interrupted` stops the wait.
    pub(crate) fn answer(
        self,
        py: Python<'_>,
        rows: RowsRequest,
        interrupted: &AtomicBool,
    ) -> PyResult<Result<RowsEvaluation, RowsError>> {
        let Self {
            speculative,
            report,
        } = self;
        report.consulted.store(true, Ordering::Release);
        let waited: Instant = Instant::now();
        let answered: SpeculativeAnswer = py
            .compiler_detach(|| Ok(answer_speculative_rows(speculative, rows, interrupted)))
            .map_err(value_error)?;
        report.wait_ns.store(
            u64::try_from(waited.elapsed().as_nanos()).unwrap_or(u64::MAX),
            Ordering::Release,
        );
        match answered {
            SpeculativeAnswer::Answered { evaluation, used } => {
                report.used.store(used, Ordering::Release);
                Ok(evaluation)
            }
            SpeculativeAnswer::Stopped => Ok(Err(RowsError::Rules(CANCELLED.to_owned()))),
        }
    }

    /// Cancel the speculation and wait for its thread without holding the interpreter.
    pub(crate) fn discard(self, py: Python<'_>) {
        let speculative: SpeculativeRows = self.speculative;
        py.detach(|| discard_speculative_rows(speculative));
    }
}
