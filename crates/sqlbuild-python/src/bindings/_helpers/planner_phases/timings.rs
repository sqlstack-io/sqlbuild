//! Wall time per top-level planning step, collected for the plan command that opened the scope.

use std::cell::RefCell;
use std::time::Instant;

use serde_json::{Value, json};

use crate::bindings::_helpers::planner_phases::constants::MILLISECONDS;
use crate::bindings::_helpers::planner_phases::constants::PERCENT;

/// Compiling the project before planning; a location planning its published project skips it.
pub(crate) const COMPILING: &str = "Compiling the project";
/// The warehouse snapshot, model migrations and source relations, live or from a synced state.
pub(crate) const READING_WAREHOUSE: &str = "Reading warehouse state";
/// Model identities, buildability and the changes each selected node has.
pub(crate) const DETECTING_CHANGES: &str = "Detecting changes";
/// Observing selected sources' freshness and comparing it to recorded state.
pub(crate) const SOURCE_FRESHNESS: &str = "Checking source freshness";
/// Reconciling changes into actions and building each node's plan entry.
pub(crate) const BUILDING_ENTRIES: &str = "Building plan entries";
/// Waiting for SQL test planning, which runs alongside the other steps.
pub(crate) const TEST_FIXTURES: &str = "Preparing test fixtures";
/// Storage policies, warnings and metadata around the plan entries.
pub(crate) const ASSEMBLING: &str = "Assembling the plan";
/// `--changed`: reading recorded consumption to decide what runs.
pub(crate) const BUILD_HISTORY: &str = "Reading build history";
/// Rendering the plan as text or JSON.
pub(crate) const WRITING: &str = "Writing the plan";
/// Time inside the plan that no step covers.
pub(crate) const OTHER: &str = "Other";

thread_local! {
    static STEPS: RefCell<Option<Vec<(&'static str, f64)>>> = const { RefCell::new(None) };
}

/// While held, this thread's planning steps are recorded for one plan command.
pub(crate) struct PlanTimingScope {
    started: Instant,
    previous: Option<Vec<(&'static str, f64)>>,
}

/// A plan's steps in pipeline order with `Other` last, and its total, in seconds.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PlanTimings {
    pub(crate) total_seconds: f64,
    pub(crate) steps: Vec<(&'static str, f64)>,
}

impl PlanTimingScope {
    pub(crate) fn enter() -> Self {
        Self {
            started: Instant::now(),
            previous: STEPS.replace(Some(Vec::new())),
        }
    }

    /// The steps recorded so far and the time since the scope opened; `Other` is the remainder.
    pub(crate) fn timings(&self) -> PlanTimings {
        let total_seconds: f64 = self.started.elapsed().as_secs_f64();
        let recorded: Vec<(&'static str, f64)> =
            STEPS.with(|steps| steps.borrow().clone().unwrap_or_default());
        timings_of(recorded, total_seconds)
    }
}

impl Drop for PlanTimingScope {
    fn drop(&mut self) {
        STEPS.set(self.previous.take());
    }
}

/// `recorded` steps within `total_seconds`, with `Other` added as what they leave uncovered.
pub(crate) fn timings_of(
    mut recorded: Vec<(&'static str, f64)>,
    total_seconds: f64,
) -> PlanTimings {
    let covered: f64 = recorded.iter().map(|(_, seconds)| seconds).sum();
    recorded.push((OTHER, (total_seconds - covered).max(0.0)));
    PlanTimings {
        total_seconds,
        steps: recorded,
    }
}

/// Add `seconds` to `step` for the plan command collecting on this thread, if one is.
pub(crate) fn record(step: &'static str, seconds: f64) {
    STEPS.with(|steps| {
        if let Some(steps) = steps.borrow_mut().as_mut() {
            match steps.iter_mut().find(|(name, _)| *name == step) {
                Some((_, total)) => *total += seconds,
                None => steps.push((step, seconds)),
            }
        }
    });
}

/// Run `body`, recording its wall time as `step`.
pub(crate) fn timed<T>(step: &'static str, body: impl FnOnce() -> T) -> T {
    let started: Instant = Instant::now();
    let result: T = body();
    record(step, started.elapsed().as_secs_f64());
    result
}

impl PlanTimings {
    /// The `--timings` table: each step's seconds and share of the total, then the total.
    pub(crate) fn table(&self) -> String {
        const HEADER: &str = "Plan timings";
        let name_width: usize = self
            .steps
            .iter()
            .map(|(name, _)| name.len() + 2)
            .max()
            .unwrap_or(0)
            .max(HEADER.len());
        let mut lines: Vec<String> = vec![format!(
            "{HEADER:<name_width$}  {:>7}  {:>5}",
            "seconds", "share"
        )];
        let row_width: usize = name_width - 2;
        for (name, seconds) in &self.steps {
            let share: f64 = Some(seconds / self.total_seconds * PERCENT)
                .filter(|share| share.is_finite())
                .unwrap_or_default();
            lines.push(format!(
                "  {name:<row_width$}  {seconds:>7.1}  {:>4.0}%",
                share
            ));
        }
        lines.push(format!(
            "  {:<row_width$}  {:>7.1}",
            "Total", self.total_seconds
        ));
        lines.join("\n")
    }

    /// The plan document's `timings`: total and steps in seconds, and what the plan read.
    pub(crate) fn json(&self, planned_from: &str) -> Value {
        let rounded = |seconds: f64| (seconds * MILLISECONDS).round() / MILLISECONDS;
        json!({
            "total_seconds": rounded(self.total_seconds),
            "steps": self
                .steps
                .iter()
                .map(|(name, seconds)| json!({"name": name, "seconds": rounded(*seconds)}))
                .collect::<Vec<Value>>(),
            "planned_from": planned_from,
        })
    }
}
