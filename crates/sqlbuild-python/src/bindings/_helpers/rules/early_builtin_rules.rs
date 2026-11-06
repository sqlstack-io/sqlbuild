//! Built-in rule evaluation started speculatively once a compile's model types are recovered.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use pyo3::exceptions::PyException;
use pyo3::prelude::{Bound, IntoPyObject, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_host::compile_context::main::add_compile_timing::add_compile_timing;
use sqlbuild_host::compile_context::main::record_compile_metric::record_compile_metric;
use sqlbuild_host::compile_context::main::record_compile_timing::record_compile_timing;
use sqlbuild_rules::engine::main::answer_speculative_rows::answer_speculative_rows;
use sqlbuild_rules::engine::main::discard_speculative_rows::discard_speculative_rows;
use sqlbuild_rules::engine::main::start_speculative_rows::start_speculative_rows;
use sqlbuild_rules::errors::RowsError;
use sqlbuild_rules::models::{RowsEvaluation, RowsRequest, SpeculativeAnswer, SpeculativeRows};

use crate::bindings::_helpers::boundary::panics::value_error;
use crate::bindings::_helpers::compiled_project::project::NativeCompiledProject;
use crate::bindings::_helpers::compiled_views::records::{
    compiled_project_facts, record_model_analyses,
};
use crate::bindings::_helpers::records::fields::field;
use crate::bindings::_helpers::rules::catalogue::references_custom_rules;
use crate::bindings::_helpers::rules::rule_dispatch::speculative_rules_request;
use crate::bindings::_helpers::rules::rule_plan::{rule_run_plan, selects_row_rules};
use crate::bindings::_helpers::rules::rules_config::load_rules_config_with_cache;
use crate::bindings::types::CompilerDetach;

const CANCELLED: &str = "native rules evaluation was cancelled";
const BUILD_PHASE: &str = "early_rules_build_ms";
const WAIT_PHASE: &str = "early_rules_wait_ms";
const HIT_METRIC: &str = "early_rules_hits";
const MISS_METRIC: &str = "early_rules_misses";

/// What one compile's early built-in rules start from.
pub(crate) struct EarlyRulesSettings {
    pub(crate) project_dir: PathBuf,
    pub(crate) dialect: String,
    pub(crate) no_sql_analysis: bool,
    /// Whether `--no-cache` turned the Rules cache off, as the rules phase does.
    pub(crate) no_cache: bool,
}

/// One compile thread's early built-in rules, from arming to the rules phase taking them.
enum Slot {
    Armed(EarlyRulesSettings),
    Started(EarlyBuiltinRules),
    Spent,
}

thread_local! {
    static SLOT: RefCell<Option<Slot>> = const { RefCell::new(None) };
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

/// Arms early built-in rules for one compile analysis on this thread; dropping it stops them.
pub(crate) struct EarlyRulesScope {
    owner: bool,
}

impl EarlyRulesScope {
    /// Claim this thread's slot unarmed; a nested compile analysis on this thread claims nothing.
    pub(crate) fn new() -> Self {
        let owner: bool = SLOT.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_some() {
                return false;
            }
            *slot = Some(Slot::Spent);
            true
        });
        Self { owner }
    }

    /// Start early rules with `settings` once the compile recovers its model types.
    pub(crate) fn arm(&self, settings: EarlyRulesSettings) {
        if !self.owner {
            return;
        }
        SLOT.with(|slot| {
            if let Some(current @ Slot::Spent) = slot.borrow_mut().as_mut() {
                *current = Slot::Armed(settings);
            }
        });
    }

    /// Cancel and join early rules this scope armed that started and were not taken.
    pub(crate) fn stop(&self, py: Python<'_>) {
        if self.owner
            && let Some(rules) = take_started()
        {
            rules.discard(py);
        }
    }
}

impl Drop for EarlyRulesScope {
    fn drop(&mut self) {
        if !self.owner {
            return;
        }
        let slot: Option<Slot> = SLOT.with(|slot| slot.borrow_mut().take());
        drop(slot);
    }
}

/// The started early rules, which only the rules phase of the arming compile takes.
pub(crate) fn take_early_rules() -> Option<EarlyBuiltinRules> {
    take_started()
}

fn take_started() -> Option<EarlyBuiltinRules> {
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

/// Start early built-in rules over `project` when armed; only non-`Exception` errors propagate.
pub(crate) fn start_early_rules(project: &Bound<'_, PyAny>) -> PyResult<()> {
    let settings: Option<EarlyRulesSettings> = SLOT.with(|slot| {
        let mut slot = slot.borrow_mut();
        match slot.as_mut() {
            Some(current @ Slot::Armed(_)) => match std::mem::replace(current, Slot::Spent) {
                Slot::Armed(settings) => Some(settings),
                _ => None,
            },
            _ => None,
        }
    });
    let Some(settings) = settings else {
        return Ok(());
    };
    let py: Python<'_> = project.py();
    let started: Option<EarlyBuiltinRules> =
        match record_compile_timing(BUILD_PHASE, || early_request(project, &settings)) {
            Ok(Some(rows)) => EarlyBuiltinRules::spawn(rows),
            Ok(None) => None,
            Err(error) if error.is_instance_of::<PyException>(py) => None,
            Err(error) => return Err(error),
        };
    if let Some(started) = started {
        SLOT.with(|slot| {
            if let Some(current) = slot.borrow_mut().as_mut() {
                *current = Slot::Started(started);
            }
        });
    }
    Ok(())
}

/// The built-in rules request the rules phase is expected to build, when it evaluates no custom rules.
fn early_request(
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
    fn spawn(rows: RowsRequest) -> Option<Self> {
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
