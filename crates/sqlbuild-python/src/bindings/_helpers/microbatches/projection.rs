//! Coverage, replay-requirement and latest-requirement projections over append-only events.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python};
use pyo3::types::{PyDict, PyString, PyTuple};
use pyo3::{IntoPyObjectExt, pyfunction, wrap_pyfunction};
use sqlbuild_host::entries::models::ExecutorHost;
use sqlbuild_state::microbatches::main::latest_requirement::latest_requirement;
use sqlbuild_state::microbatches::main::project_coverage::project_coverage;
use sqlbuild_state::microbatches::main::project_replay::project_replay;
use sqlbuild_state::microbatches::models::{
    BoundInterval, CompletionFact, CoveragePiece, CoverageProjection, ReplayProjection,
    ReplayState, ReplayVersions, RequirementFact, TextInterval,
};

use crate::bindings::_helpers::microbatches::conversion::{
    BoundParser, enum_text, optional_text, timestamp,
};
use sqlbuild_python_base::boundary::main::call_with::call_with;
use sqlbuild_python_base::boundary::models::Request;
use sqlbuild_python_base::native_host::main::host_entry::host_entry;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const COMPLETION_RECORD_TYPES: [&str; 2] = ["partition_completion", "synthetic_completion"];
const ORDINARY_COMPLETION: &str = "partition_completion";
const REPLAY_REQUIREMENT: &str = "replay_requirement";
const UNKNOWN_FINGERPRINT: &str = "unknown";

fn python_intervals<'py>(
    py: Python<'py>,
    intervals: &[TextInterval],
) -> PyResult<Bound<'py, PyTuple>> {
    let class: Bound<'py, PyAny> = host_entry(py, ExecutorHost::MicrobatchInterval)?;
    let built: Vec<Bound<'py, PyAny>> = intervals
        .iter()
        .map(|interval| {
            call_with(
                &class,
                &[
                    ("start", PyString::new(py, &interval.start).into_any()),
                    ("end", PyString::new(py, &interval.end).into_any()),
                ],
            )
        })
        .collect::<PyResult<_>>()?;
    PyTuple::new(py, built)
}

fn python_coverage<'py>(
    py: Python<'py>,
    projection: &CoverageProjection,
    events: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    let projected_class: Bound<'py, PyAny> =
        host_entry(py, ExecutorHost::ProjectedMicrobatchInterval)?;
    let mut projected: Vec<Bound<'py, PyAny>> = Vec::with_capacity(projection.segments.len());
    for segment in &projection.segments {
        let event: &Bound<'py, PyAny> = &events[segment.completion];
        projected.push(call_with(
            &projected_class,
            &[
                (
                    "start",
                    PyString::new(py, &segment.interval.start).into_any(),
                ),
                ("end", PyString::new(py, &segment.interval.end).into_any()),
                (
                    "fingerprint_status",
                    native_field(event, "fingerprint_status")?,
                ),
                (
                    "model_version_hash",
                    native_field(event, "model_version_hash")?,
                ),
                ("record_type", native_field(event, "record_type")?),
                ("completion_type", native_field(event, "completion_type")?),
                ("event_id", native_field(event, "event_id")?),
            ],
        )?);
    }
    call_with(
        &host_entry(py, ExecutorHost::MicrobatchCoverageProjection)?,
        &[
            ("intervals", PyTuple::new(py, projected)?.into_any()),
            (
                "contiguous_frontier",
                projection
                    .contiguous_frontier
                    .as_deref()
                    .into_bound_py_any(py)?,
            ),
            (
                "known_missing",
                python_intervals(py, &projection.known_missing)?.into_any(),
            ),
            (
                "unaccounted",
                python_intervals(py, &projection.unaccounted)?.into_any(),
            ),
            (
                "unknown_fingerprints",
                python_intervals(py, &projection.unknown_fingerprints)?.into_any(),
            ),
        ],
    )
}

/// Fold completion facts and classify uncovered expected intervals.
#[pyfunction(name = "project_microbatch_coverage")]
#[pyo3(signature = (*, events, expected_intervals, cursor_type))]
fn coverage<'py>(
    events: &Bound<'py, PyAny>,
    expected_intervals: &Bound<'py, PyAny>,
    cursor_type: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = events.py();
    let mut parser: BoundParser<'py> = BoundParser::new(cursor_type)?;
    let mut completion_events: Vec<Bound<'py, PyAny>> = Vec::new();
    let mut completions: Vec<CompletionFact> = Vec::new();
    for event in events.try_iter()? {
        let event: Bound<'py, PyAny> = event?;
        let record_type: String = enum_text(&native_field(&event, "record_type")?)?;
        let (start, end) = (
            native_field(&event, "partition_start")?,
            native_field(&event, "partition_end")?,
        );
        if !COMPLETION_RECORD_TYPES.contains(&record_type.as_str())
            || start.is_none()
            || end.is_none()
        {
            continue;
        }
        completions.push(CompletionFact {
            interval: BoundInterval {
                start: parser.bound(&start)?,
                end: parser.bound(&end)?,
            },
            ordinary: record_type == ORDINARY_COMPLETION,
            unknown_fingerprint: enum_text(&native_field(&event, "fingerprint_status")?)?
                == UNKNOWN_FINGERPRINT,
            created: timestamp(&native_field(&event, "created_at")?)?,
            event_id: native_field(&event, "event_id")?.extract()?,
        });
        completion_events.push(event);
    }
    let expected: Vec<BoundInterval> = expected_intervals
        .try_iter()?
        .map(|interval| parser.interval(&interval?))
        .collect::<PyResult<_>>()?;
    python_coverage(
        py,
        &project_coverage(&completions, &expected),
        &completion_events,
    )
}

fn replay_state_member(state: ReplayState) -> &'static str {
    match state {
        ReplayState::Incomplete => "INCOMPLETE",
        ReplayState::VerifiedComplete => "VERIFIED_COMPLETE",
        ReplayState::CompleteWithUnknownFingerprints => "COMPLETE_WITH_UNKNOWN_FINGERPRINTS",
        ReplayState::Superseded => "SUPERSEDED",
    }
}

/// Derive replay completion without writing mutable requirement status.
#[pyfunction(name = "project_replay_requirement")]
#[pyo3(signature = (**request))]
fn replay<'py>(request: Option<&Bound<'py, PyDict>>) -> PyResult<Bound<'py, PyAny>> {
    let request = Request::new("project_replay_requirement", request)?;
    let py: Python<'py> = request.py();
    let requirement: Bound<'py, PyAny> = request.get("requirement")?;
    let current: String = request.get("current_model_version_hash")?.extract()?;
    let required: Option<String> =
        optional_text(&native_field(&requirement, "required_model_version_hash")?)?;
    let versions = ReplayVersions {
        required: required.as_deref(),
        current: &current,
    };
    let expected_objects: Vec<Bound<'py, PyAny>> = request
        .get("expected_intervals")?
        .try_iter()?
        .collect::<PyResult<_>>()?;
    let mut expected: Vec<BoundInterval> = Vec::new();
    let mut pieces: Vec<CoveragePiece> = Vec::new();
    if required.as_deref() == Some(current.as_str()) {
        let mut parser: BoundParser<'py> = BoundParser::new(&request.get("cursor_type")?)?;
        for interval in &expected_objects {
            expected.push(parser.interval(interval)?);
        }
        for piece in native_field(&request.get("coverage")?, "intervals")?.try_iter()? {
            let piece: Bound<'py, PyAny> = piece?;
            pieces.push(CoveragePiece {
                interval: parser.interval(&piece)?,
                unknown_fingerprint: enum_text(&native_field(&piece, "fingerprint_status")?)?
                    == UNKNOWN_FINGERPRINT,
                model_version_hash: optional_text(&native_field(&piece, "model_version_hash")?)?,
            });
        }
    }
    let projection: ReplayProjection = project_replay(versions, &expected, &pieces);
    let selected = |indexes: &[usize]| {
        PyTuple::new(
            py,
            indexes.iter().map(|index| expected_objects[*index].clone()),
        )
    };
    call_with(
        &host_entry(py, ExecutorHost::ReplayRequirementProjection)?,
        &[
            ("requirement", requirement),
            (
                "state",
                host_entry(py, ExecutorHost::ReplayRequirementState)?
                    .getattr(replay_state_member(projection.state))?,
            ),
            ("missing", selected(&projection.missing)?.into_any()),
            (
                "unknown_fingerprints",
                selected(&projection.unknown_fingerprints)?.into_any(),
            ),
        ],
    )
}

/// Return only the latest requirement created for the currently compiled version.
#[pyfunction(name = "latest_active_replay_requirement")]
#[pyo3(signature = (*, events, current_model_version_hash))]
fn latest<'py>(
    events: &Bound<'py, PyAny>,
    current_model_version_hash: &str,
) -> PyResult<Option<Bound<'py, PyAny>>> {
    let objects: Vec<Bound<'py, PyAny>> = events.try_iter()?.collect::<PyResult<_>>()?;
    let mut facts: Vec<RequirementFact> = Vec::with_capacity(objects.len());
    for event in &objects {
        let is_requirement: bool =
            enum_text(&native_field(event, "record_type")?)? == REPLAY_REQUIREMENT;
        let required: Option<String> =
            optional_text(&native_field(event, "required_model_version_hash")?)?;
        let matching: bool =
            is_requirement && required.as_deref() == Some(current_model_version_hash);
        facts.push(RequirementFact {
            is_requirement,
            required_model_version_hash: required,
            created: if matching {
                timestamp(&native_field(event, "created_at")?)?
            } else {
                0.0
            },
            event_id: native_field(event, "event_id")?.extract()?,
        });
    }
    Ok(latest_requirement(&facts, current_model_version_hash).map(|index| objects[index].clone()))
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(coverage, module)?)?;
    module.add_function(wrap_pyfunction!(replay, module)?)?;
    module.add_function(wrap_pyfunction!(latest, module)?)
}
