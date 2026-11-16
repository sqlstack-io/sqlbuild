//! Process-wide slots for what a full compile captures and what a delta compile injects.

use crate::compile_context::constants::ERRORS_ON_DELTA_PATH;
use crate::compile_context::models::DeltaInjection;
use crate::compile_context::models::ImageCapture;
use crate::compile_context::models::RecordMerge;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use pyo3::prelude::PyResult;
use sqlbuild_analysis::assembly::analysis_session::models::{
    DynamicFamily, RelationTables, SessionRequest,
};
use sqlbuild_cache::project_reuse::types::ImageSections;

static CAPTURE: Mutex<Option<ImageCapture>> = Mutex::new(None);
static INJECTION: Mutex<Option<Arc<DeltaInjection>>> = Mutex::new(None);

/// Why the running delta compile cannot continue, once something ruled it out.
static UNSUPPORTED: Mutex<Option<String>> = Mutex::new(None);
static PENDING_IMAGE: Mutex<Option<ImageSections>> = Mutex::new(None);
static RECORD_MERGE: Mutex<Option<RecordMerge>> = Mutex::new(None);

fn locked<T>(slot: &Mutex<T>) -> MutexGuard<'_, T> {
    slot.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Start capturing image facts for the compile about to run.
pub(crate) fn start_capture() {
    *locked(&CAPTURE) = Some(ImageCapture::default());
}

/// Stop capturing without keeping anything, for a project that turns delta compiles off.
pub(crate) fn stop_capture() {
    *locked(&CAPTURE) = None;
}

/// Stop capturing and take what was captured.
pub(crate) fn take_capture() -> Option<ImageCapture> {
    locked(&CAPTURE).take()
}

/// Whether the running compile captures image facts.
pub(crate) fn capturing() -> bool {
    locked(&CAPTURE).is_some()
}

/// Record into the capture, when one is running.
pub(crate) fn capture(record: impl FnOnce(&mut ImageCapture)) {
    if let Some(capture) = locked(&CAPTURE).as_mut() {
        record(capture);
    }
}

/// Record sources a resource other than a model reads, when a capture is running.
pub(crate) fn record_source_reads<'a>(names: impl IntoIterator<Item = &'a str>) {
    capture(|capture| {
        capture
            .pinned_sources
            .extend(names.into_iter().map(str::to_owned));
    });
}

/// Record sources an SQL test file reads, when a capture is running.
pub(crate) fn record_test_source_reads<'a>(
    relative_path: &str,
    names: impl IntoIterator<Item = &'a str>,
) {
    capture(|capture| {
        capture
            .test_sources
            .entry(relative_path.replace('\\', "/"))
            .or_default()
            .extend(names.into_iter().map(str::to_owned));
    });
}

/// Mark the running capture as unusable as a delta base.
pub(crate) fn capture_unsupported(reason: &str) {
    capture(|capture| {
        if capture.unsupported.is_none() {
            capture.unsupported = Some(reason.to_owned());
        }
    });
}

/// Run `body` with `injection` visible to the rerun stages; returns it unless still held.
pub(crate) fn with_injection<T>(
    injection: DeltaInjection,
    body: impl FnOnce() -> T,
) -> (T, Option<DeltaInjection>) {
    *locked(&INJECTION) = Some(Arc::new(injection));
    let result: T = body();
    let injection: Option<Arc<DeltaInjection>> = locked(&INJECTION).take();
    (
        result,
        injection.and_then(|injection| {
            let Ok(injection) = Arc::try_unwrap(injection) else {
                return None;
            };
            Some(injection)
        }),
    )
}

/// The running delta compile's injection, if any.
pub(crate) fn injection() -> Option<Arc<DeltaInjection>> {
    locked(&INJECTION).clone()
}

/// Rule out the running delta compile: returns the error that unwinds to its fallback.
pub(crate) fn delta_unsupported(reason: &str) -> pyo3::PyErr {
    let mut slot = locked(&UNSUPPORTED);
    if slot.is_none() {
        *slot = Some(reason.to_owned());
    }
    pyo3::exceptions::PyRuntimeError::new_err(format!("delta compile unsupported: {reason}"))
}

/// In a delta compile that leaves errors to full compiles, stop once `failed` reports one.
pub(crate) fn stop_on_errors(failed: bool) -> pyo3::PyResult<()> {
    let errors_on_delta_path: bool = ERRORS_ON_DELTA_PATH;
    if failed && !errors_on_delta_path && injection().is_some() {
        return Err(delta_unsupported("the edit reports errors"));
    }
    Ok(())
}

/// Take the reason the delta compile was ruled out, if any.
pub(crate) fn take_delta_unsupported() -> Option<String> {
    locked(&UNSUPPORTED).take()
}

/// The previous compile's digest of a custom-rule fact about a resource the delta did not rerun.
pub(crate) fn injected_fact_digest(key: &[String]) -> Option<String> {
    let injection: Arc<DeltaInjection> = injection()?;
    if key
        .iter()
        .skip(1)
        .any(|part| injection.rerun_names.contains(part))
    {
        return None;
    }
    injection.fact_digests.get(key).cloned()
}

/// Give a delta's analysis session the whole project's relation facts and analysis order.
pub(crate) fn inject_relations(mut request: SessionRequest) -> pyo3::PyResult<SessionRequest> {
    let Some(injection) = injection() else {
        return Ok(request);
    };
    let relations: &RelationTables = &injection.relations;
    request.forced_order = Some((relations.dependency_ordered, relations.cyclic));
    request.retain_relations = true;
    let analysed: BTreeSet<&str> = request
        .models
        .iter()
        .map(|model| model.name.as_str())
        .collect();
    let rerun: BTreeSet<&str> = analysed
        .iter()
        .copied()
        .chain(injection.rerun_names.iter().map(String::as_str))
        .collect();
    request.dynamic_families_by_table = injected_families(
        &request.dynamic_families_by_table,
        &relations.families,
        &rerun,
    )?;
    for (table, injected) in [
        (&mut request.column_types, &relations.types),
        (&mut request.column_nullability, &relations.nullability),
        (&mut request.complete_schemas, &relations.complete),
    ] {
        let present: BTreeSet<String> = table.iter().map(|(name, _)| name.clone()).collect();
        table.extend(
            injected
                .iter()
                .filter(|(name, _)| !analysed.contains(name.as_str()) && !present.contains(name))
                .cloned(),
        );
    }
    Ok(request)
}

/// The stored compile's dynamic column families, when no rerun model changed its own.
fn injected_families(
    current: &[(String, Vec<DynamicFamily>)],
    stored: &[(String, Vec<DynamicFamily>)],
    rerun: &BTreeSet<&str>,
) -> pyo3::PyResult<Vec<(String, Vec<DynamicFamily>)>> {
    let families_of = |table: &[(String, Vec<DynamicFamily>)], name: &str| {
        table
            .iter()
            .find(|(table, _)| table == name)
            .map(|(_, families)| families.clone())
    };
    let changed: bool = current
        .iter()
        .any(|(name, _)| !rerun.contains(name.as_str()))
        || rerun
            .iter()
            .any(|name| families_of(current, name) != families_of(stored, name));
    if changed {
        return Err(delta_unsupported(
            "a model's dynamic column families changed",
        ));
    }
    Ok(stored.to_vec())
}

/// Hand the next reuse record the image to keep beside it.
pub(crate) fn set_pending_image(image: Option<ImageSections>) {
    *locked(&PENDING_IMAGE) = image;
}

/// The image the next reuse record keeps, taken.
pub(crate) fn take_pending_image() -> Option<ImageSections> {
    locked(&PENDING_IMAGE).take()
}

/// Hand the next reuse record what it keeps of the patched compile.
pub(crate) fn set_record_merge(merge: Option<RecordMerge>) {
    *locked(&RECORD_MERGE) = merge;
}

/// What the next reuse record keeps of the patched compile, taken.
pub(crate) fn take_record_merge() -> Option<RecordMerge> {
    locked(&RECORD_MERGE).take()
}

/// Forget every capture, injection and pending record fact, after a compile ends either way.
pub(crate) fn reset() {
    let _ = take_capture();
    *locked(&INJECTION) = None;
    let _ = take_delta_unsupported();
    set_pending_image(None);
    set_record_merge(None);
}

/// Run a capture hook; a failure only rules the compile out as a delta base.
pub(crate) fn guarded(hook: impl FnOnce() -> PyResult<()>) -> PyResult<()> {
    if !capturing() {
        return Ok(());
    }
    if let Err(error) = hook() {
        capture_unsupported(&format!("an image fact could not be recorded: {error}"));
    }
    Ok(())
}
